//! Plugin subprocess lifecycle: spawn, send, drain, close.
//!
//! Each active process plugin gets a `Plugin` struct holding the child process
//! handle plus background reader (stdout → action channel) and writer (send
//! queue → stdin) threads.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::plugin::types::{FromPlugin, ToPlugin};

/// Maximum line length from a plugin's stdout (64 KB). Lines exceeding this
/// are discarded and the reader continues.
pub(crate) const MAX_LINE_LEN: usize = 65536;

/// A single running plugin subprocess with background reader and writer threads.
pub(crate) struct Plugin {
    pub(crate) name: String,
    /// Events this plugin subscribes to. Empty = all events (backward compat).
    subscribed_events: Vec<String>,
    child: Option<Child>,
    /// Sends serialised JSON lines to the plugin's stdin via the writer thread.
    write_tx: Option<std::sync::mpsc::SyncSender<String>>,
    action_rx: Option<std::sync::mpsc::Receiver<(String, serde_json::Value)>>,
    _reader_thread: Option<std::thread::JoinHandle<()>>,
    _writer_thread: Option<std::thread::JoinHandle<()>>,
}

impl Plugin {
    pub(crate) fn new(name: String, subscribed_events: Vec<String>) -> Self {
        Plugin {
            name,
            subscribed_events,
            child: None,
            write_tx: None,
            action_rx: None,
            _reader_thread: None,
            _writer_thread: None,
        }
    }

    /// Returns `true` if this plugin has subscribed to the given event.
    /// Empty subscription list means all events are accepted (backward compat).
    pub(crate) fn subscribes_to(&self, event: &str) -> bool {
        self.subscribed_events.is_empty() || self.subscribed_events.iter().any(|e| e == event)
    }

    pub(crate) fn spawn(&mut self, path: &Path) -> Result<(), String> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn plugin '{}': {}", self.name, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("no stdin for plugin '{}'", self.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("no stdout for plugin '{}'", self.name))?;

        let (read_tx, read_rx) = std::sync::mpsc::sync_channel(1024);
        let name_for_reader = self.name.clone();
        let read_handle = std::thread::Builder::new()
            .name(format!("plugin-reader-{}", name_for_reader))
            .spawn(move || {
                let mut reader = std::io::BufReader::new(stdout);
                let mut line_buf: Vec<u8> = Vec::with_capacity(1024);
                loop {
                    line_buf.clear();
                    // Read up to MAX_LINE_LEN bytes looking for '\n'
                    let got_newline = read_capped_line(&mut reader, &mut line_buf);
                    if line_buf.is_empty() {
                        break;
                    }
                    // Discard lines exceeding the cap (no newline at MAX_LINE_LEN)
                    if !got_newline {
                        drain_rest_of_line(&mut reader);
                        continue;
                    }
                    if line_buf.ends_with(b"\n") {
                        line_buf.pop();
                    }
                    let trimmed = String::from_utf8_lossy(&line_buf).trim().to_string();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(msg) = serde_json::from_str::<FromPlugin>(&trimmed) {
                        if msg.event == "action" {
                            if let Some(action) = msg.action {
                                let _ = read_tx.try_send((action, msg.params));
                            }
                        }
                    }
                }
            })
            .map_err(|e| format!("failed to spawn reader thread: {e}"))?;

        let (write_tx, write_rx) = std::sync::mpsc::sync_channel::<String>(1024);
        let name_for_writer = self.name.clone();
        let write_handle = std::thread::Builder::new()
            .name(format!("plugin-writer-{}", name_for_writer))
            .spawn(move || {
                let mut stdin = stdin;
                for msg in write_rx {
                    if writeln!(stdin, "{msg}").is_err() {
                        break;
                    }
                    let _ = stdin.flush();
                }
            })
            .map_err(|e| format!("failed to spawn writer thread: {e}"))?;

        self.child = Some(child);
        self.write_tx = Some(write_tx);
        self.action_rx = Some(read_rx);
        self._reader_thread = Some(read_handle);
        self._writer_thread = Some(write_handle);
        Ok(())
    }

    /// Enqueues a message for the writer thread; drops on full (never blocks).
    pub(crate) fn send(&mut self, msg: &ToPlugin) {
        let Some(ref write_tx) = self.write_tx else {
            return;
        };
        let Ok(json) = serde_json::to_string(msg) else {
            return;
        };
        let _ = write_tx.try_send(json);
    }

    pub(crate) fn drain_actions(&mut self) -> Vec<(String, serde_json::Value)> {
        let mut actions = Vec::new();
        let Some(ref rx) = self.action_rx else {
            return actions;
        };
        while let Ok((action, params)) = rx.try_recv() {
            actions.push((action, params));
        }
        actions
    }

    /// Drops the write channel (so the writer thread flushes and exits), then
    /// waits up to 2 s for the child to exit before killing it.
    #[allow(dead_code)]
    pub(crate) fn close(&mut self) {
        self.close_with_timeout(Duration::from_secs(2));
    }

    fn close_with_timeout(&mut self, timeout: Duration) {
        drop(self.write_tx.take());
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + timeout;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() >= deadline => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    _ => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        }
    }

    /// Non-blocking shutdown: drops stdin (signals the plugin to exit), then
    /// moves the child process into a background thread that waits up to 2 s
    /// for a clean exit before force-killing it. The reader/writer thread
    /// handles are dropped here and exit naturally as their channels close.
    pub(crate) fn close_in_background(mut self) {
        drop(self.write_tx.take());
        if let Some(child) = self.child.take() {
            let name = self.name.clone();
            let child = std::sync::Arc::new(std::sync::Mutex::new(child));
            let bg = child.clone();
            if std::thread::Builder::new()
                .name(format!("plugin-closer-{name}"))
                .spawn(move || {
                    let mut c = bg.lock().unwrap_or_else(|e| e.into_inner());
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        match c.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if Instant::now() >= deadline => {
                                let _ = c.kill();
                                let _ = c.wait();
                                break;
                            }
                            _ => std::thread::sleep(Duration::from_millis(50)),
                        }
                    }
                })
                .is_err()
            {
                if let Ok(mut c) = child.lock() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        }
    }
}

/// Reads up to `MAX_LINE_LEN` bytes from `reader` into `buf`, stopping at the
/// first `\n`. Returns `true` if `buf` holds a complete line to process — i.e.
/// a newline was found within the limit, or EOF/error was reached with bytes
/// already buffered (the final unterminated line). Returns `false` only when
/// the line was truncated at `MAX_LINE_LEN` without a newline, or nothing was
/// read before EOF.
pub(crate) fn read_capped_line<R: BufRead>(reader: &mut R, buf: &mut Vec<u8>) -> bool {
    loop {
        let (available_len, has_newline) = {
            let available = match reader.fill_buf() {
                Ok([]) => return !buf.is_empty(),
                Ok(b) => b,
                Err(_) => return !buf.is_empty(),
            };
            let remaining = MAX_LINE_LEN.saturating_sub(buf.len());
            if remaining == 0 {
                return false;
            }
            let newline_pos = available.iter().position(|&b| b == b'\n');
            match newline_pos {
                // Newline lies within the cap: take the full line incl. '\n'.
                Some(pos) if pos < remaining => {
                    buf.extend_from_slice(&available[..pos + 1]);
                    (pos + 1, true)
                }
                // No newline within the cap window (either none in this chunk,
                // or it sits beyond `remaining`): take only what fits and keep
                // reading. The cap is enforced by `remaining == 0` above, which
                // returns `false` so the caller drains the rest of the line.
                _ => {
                    let to_read = available.len().min(remaining);
                    buf.extend_from_slice(&available[..to_read]);
                    (to_read, false)
                }
            }
        };
        reader.consume(available_len);
        if has_newline {
            return true;
        }
    }
}

/// Advances `reader` past any remaining bytes in the current line (everything
/// up to and including the next `\n`, or EOF).
pub(crate) fn drain_rest_of_line<R: BufRead>(reader: &mut R) {
    loop {
        let (consume, found_newline) = {
            let available = match reader.fill_buf() {
                Ok([]) => return,
                Ok(b) => b,
                Err(_) => return,
            };
            match available.iter().position(|&b| b == b'\n') {
                // Consume up to and including the newline, then stop — the next
                // line must be left intact for the reader loop.
                Some(pos) => (pos + 1, true),
                // No newline yet: consume the chunk and keep reading.
                None => (available.len(), false),
            }
        };
        reader.consume(consume);
        if found_newline {
            return;
        }
    }
}
