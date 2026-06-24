//! Plugin subprocess lifecycle: spawn, send, drain, close.
//!
//! Each active process plugin gets a `Plugin` struct holding the child process
//! handle plus background reader (stdout → action channel) and writer (send
//! queue → stdin) threads.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use crate::plugin::types::{FromPlugin, ToPlugin};

/// A single running plugin subprocess with background reader and writer threads.
pub(crate) struct Plugin {
    pub(crate) name: String,
    child: Option<Child>,
    /// Sends serialised JSON lines to the plugin's stdin via the writer thread.
    write_tx: Option<Sender<String>>,
    action_rx: Option<std::sync::mpsc::Receiver<(String, serde_json::Value)>>,
    _reader_thread: Option<std::thread::JoinHandle<()>>,
    _writer_thread: Option<std::thread::JoinHandle<()>>,
}

impl Plugin {
    pub(crate) fn new(name: String) -> Self {
        Plugin {
            name,
            child: None,
            write_tx: None,
            action_rx: None,
            _reader_thread: None,
            _writer_thread: None,
        }
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
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if let Ok(msg) = serde_json::from_str::<FromPlugin>(trimmed) {
                                if msg.event == "action" {
                                    if let Some(action) = msg.action {
                                        let _ = read_tx.try_send((action, msg.params));
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn reader thread: {e}"))?;

        let (write_tx, write_rx) = std::sync::mpsc::channel::<String>();
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

    /// Enqueues a message for the writer thread; never blocks the caller.
    pub(crate) fn send(&mut self, msg: &ToPlugin) {
        let Some(ref write_tx) = self.write_tx else {
            return;
        };
        let Ok(json) = serde_json::to_string(msg) else {
            return;
        };
        let _ = write_tx.send(json);
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
