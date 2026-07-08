//! Per-session timestamped JSONL sink for telemetry events.
//!
//! Each session writes to its own timestamped file
//! `events-<session-epoch>.jsonl` (where session-epoch is the `SystemTime`
//! epoch second at session start), so every file maps 1:1 to a mantis session
//! and the filename alone tells you which session's data is inside.  When the
//! active file would exceed the size cap it is renamed to
//! `events-<session-epoch>-<n>.jsonl` and a fresh active file with the same
//! session-epoch prefix is started; the oldest rotated files are pruned so
//! total disk use stays bounded regardless of how long telemetry stays enabled.
//! All I/O is best-effort: a broken state dir degrades to silently dropped
//! events, never an error surfaced to the UI. Owned by [`super::Telemetry`];
//! not public outside the telemetry module.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use super::TelemetryEvent;

/// Rotate the active file once it would exceed this many bytes.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Keep at most this many rotated files (the active file is extra).
const MAX_ROTATED_FILES: usize = 4;

pub(crate) struct JsonlSink {
    dir: PathBuf,
    /// Basename of the active file (e.g. `events-1770393600.jsonl`).
    active_name: String,
    /// Epoch second from the session start, used for all file names in this
    /// session.
    session_epoch: u64,
    active_len: u64,
    max_file_bytes: u64,
    max_rotated: usize,
}

impl JsonlSink {
    pub(crate) fn new(dir: PathBuf, session_epoch: u64) -> Self {
        Self::with_limits(dir, session_epoch, MAX_FILE_BYTES, MAX_ROTATED_FILES)
    }

    /// Constructor with explicit caps so tests can force rotation cheaply.
    pub(crate) fn with_limits(
        dir: PathBuf,
        session_epoch: u64,
        max_file_bytes: u64,
        max_rotated: usize,
    ) -> Self {
        let _ = fs::create_dir_all(&dir);
        let mut active_name = format!("events-{session_epoch}.jsonl");
        let mut n = 1;
        while dir.join(&active_name).exists() {
            active_name = format!("events-{session_epoch}-{n}.jsonl");
            n += 1;
        }
        JsonlSink {
            dir,
            active_name,
            session_epoch,
            active_len: 0,
            max_file_bytes,
            max_rotated,
        }
    }

    /// Appends `event` as one JSON line stamped with session-relative `ts_ms`,
    /// rotating first when the line would push the active file over the cap.
    pub(crate) fn append(&mut self, event: &TelemetryEvent, elapsed: Duration) {
        let Ok(mut value) = serde_json::to_value(event) else {
            return;
        };
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                "ts_ms".into(),
                u64::try_from(elapsed.as_millis())
                    .unwrap_or(u64::MAX)
                    .into(),
            );
        }
        let mut line = value.to_string();
        line.push('\n');
        if self.active_len + line.len() as u64 > self.max_file_bytes {
            self.rotate();
        }
        let path = self.dir.join(&self.active_name);
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            if file.write_all(line.as_bytes()).is_ok() {
                self.active_len += line.len() as u64;
            }
        }
    }

    /// Renames the active file to `events-<session_epoch>-<n>.jsonl` and
    /// starts a fresh active file, then prunes the oldest rotated files.
    fn rotate(&mut self) {
        let old_name = std::mem::take(&mut self.active_name);
        let epoch = self.session_epoch;
        let mut n = 1;
        loop {
            let candidate = format!("events-{epoch}-{n}.jsonl");
            if candidate != old_name && !self.dir.join(&candidate).exists() {
                self.active_name = candidate;
                break;
            }
            n += 1;
        }
        let active = self.dir.join(&old_name);
        let target = self.dir.join(&self.active_name);
        let _ = fs::rename(&active, &target);
        self.active_name = format!("events-{epoch}.jsonl");
        self.active_len = 0;
        self.prune();
    }

    /// Deletes the oldest rotated files until at most `max_rotated` remain.
    /// Epoch-second names sort chronologically as strings.
    fn prune(&self) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut rotated: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("events-") && n.ends_with(".jsonl"))
            })
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n != self.active_name)
            })
            .collect();
        rotated.sort();
        while rotated.len() > self.max_rotated {
            let oldest = rotated.remove(0);
            let _ = fs::remove_file(oldest);
        }
    }
}

#[cfg(test)]
#[path = "sink_test.rs"]
mod tests;
