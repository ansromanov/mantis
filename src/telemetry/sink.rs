//! Size-capped, rotated JSONL sink for telemetry events.
//!
//! The writer thread appends one JSON object per line to `events.jsonl`
//! inside the telemetry directory, stamping each with `ts_ms` (milliseconds
//! since session start, deliberately session-relative so no wall-clock
//! timestamps leave the event loop). When the active file would exceed the
//! size cap it is renamed to `events-<epoch>.jsonl` and a fresh active file
//! is started; the oldest rotated files are pruned so total disk use stays
//! bounded regardless of how long telemetry stays enabled. All I/O is
//! best-effort: a broken state dir degrades to silently dropped events, never
//! an error surfaced to the UI. Owned by [`super::Telemetry`]; not public
//! outside the telemetry module.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::TelemetryEvent;

/// File currently being appended to.
const ACTIVE_FILE: &str = "events.jsonl";
/// Rotate the active file once it would exceed this many bytes.
const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Keep at most this many rotated files (the active file is extra).
const MAX_ROTATED_FILES: usize = 4;

pub(crate) struct JsonlSink {
    dir: PathBuf,
    active_len: u64,
    max_file_bytes: u64,
    max_rotated: usize,
}

impl JsonlSink {
    pub(crate) fn new(dir: PathBuf) -> Self {
        Self::with_limits(dir, MAX_FILE_BYTES, MAX_ROTATED_FILES)
    }

    /// Constructor with explicit caps so tests can force rotation cheaply.
    pub(crate) fn with_limits(dir: PathBuf, max_file_bytes: u64, max_rotated: usize) -> Self {
        let _ = fs::create_dir_all(&dir);
        let active_len = fs::metadata(dir.join(ACTIVE_FILE))
            .map(|m| m.len())
            .unwrap_or(0);
        JsonlSink {
            dir,
            active_len,
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
        let path = self.dir.join(ACTIVE_FILE);
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            if file.write_all(line.as_bytes()).is_ok() {
                self.active_len += line.len() as u64;
            }
        }
    }

    /// Renames the active file to a unique `events-<epoch>.jsonl` and prunes
    /// the oldest rotated files beyond the cap.
    fn rotate(&mut self) {
        let active = self.dir.join(ACTIVE_FILE);
        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let mut target = self.dir.join(format!("events-{epoch}.jsonl"));
        let mut n = 1;
        while target.exists() {
            target = self.dir.join(format!("events-{epoch}-{n}.jsonl"));
            n += 1;
        }
        let _ = fs::rename(&active, &target);
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
