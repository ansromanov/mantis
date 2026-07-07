//! Opt-in, local-only usage telemetry (disabled by default).
//!
//! When `[telemetry] enabled = true` is set in `mantis.toml`, whitelisted
//! usage events are appended as JSON lines to a size-capped, rotated sink
//! under `<state_dir>/telemetry/` (see [`crate::session::state_dir`]). Nothing
//! is ever sent anywhere: this module is the local-collection groundwork for a
//! future, separately-gated remote sink. Events are a closed enum
//! ([`TelemetryEvent`]) so the schema is a whitelist by construction — no
//! paths, filenames, file content, or typed text can be recorded. Raw
//! keystrokes are never captured; only resolved action ids are. Recording is
//! non-blocking: [`Telemetry::record`] does a `try_send` onto a bounded
//! channel drained by a background writer thread, dropping events (counted,
//! reported in `SessionEnd`) rather than ever stalling the render loop. When
//! disabled, the handle is a no-op: no thread is spawned and no files are
//! created. Public items: [`Telemetry`], [`TelemetryEvent`], [`ActionSource`].

mod sink;

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;

use sink::JsonlSink;

/// How an action was invoked. Only palette dispatch is instrumented so far;
/// key and mouse sources are added when those choke points are wired.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSource {
    Palette,
}

/// One telemetry event. A closed enum of typed fields is the privacy
/// boundary: every variant holds only version constants, whitelisted
/// environment facts, static action ids, or counters — never free-form
/// strings derived from user input or the filesystem.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum TelemetryEvent {
    SessionStart {
        app_version: &'static str,
        os: &'static str,
        arch: &'static str,
        /// Value of `$TERM` (a terminal type like `xterm-256color`, not
        /// user data); empty when unset.
        terminal: String,
    },
    SessionEnd {
        duration_s: u64,
        events_dropped: u64,
    },
    ActionInvoked {
        /// Canonical action id from [`crate::actions::ACTIONS`].
        action: &'static str,
        source: ActionSource,
    },
}

/// Bounded queue between the render loop and the writer thread. Sized so a
/// burst of palette commands never blocks; overflow drops events instead.
const CHANNEL_CAPACITY: usize = 256;

/// Handle owned by `App`. Cheap to call from anywhere; all I/O happens on the
/// background writer thread. Dropping the handle emits `SessionEnd` and joins
/// the writer so buffered events are flushed on every exit path.
pub struct Telemetry {
    inner: Option<Inner>,
}

struct Inner {
    tx: Option<SyncSender<TelemetryEvent>>,
    dropped: Arc<AtomicU64>,
    started: Instant,
    writer: Option<JoinHandle<()>>,
}

impl Telemetry {
    /// No-op handle: records nothing, spawns nothing, creates no files.
    pub fn disabled() -> Self {
        Telemetry { inner: None }
    }

    /// Production constructor: enabled telemetry writes under
    /// `<state_dir>/telemetry/`; disabled (or no usable state dir) is a no-op.
    pub fn new(enabled: bool) -> Self {
        if !enabled {
            return Self::disabled();
        }
        match crate::session::state_dir() {
            Some(dir) => Self::with_dir(dir.join("telemetry")),
            None => Self::disabled(),
        }
    }

    /// Enabled handle writing into `dir` (exposed for tests to use an
    /// isolated directory). Emits `SessionStart` immediately.
    pub fn with_dir(dir: PathBuf) -> Self {
        let started = Instant::now();
        let (tx, rx) = mpsc::sync_channel::<TelemetryEvent>(CHANNEL_CAPACITY);
        let writer = std::thread::spawn(move || {
            let mut sink = JsonlSink::new(dir);
            while let Ok(event) = rx.recv() {
                sink.append(&event, started.elapsed());
            }
        });
        let telemetry = Telemetry {
            inner: Some(Inner {
                tx: Some(tx),
                dropped: Arc::new(AtomicU64::new(0)),
                started,
                writer: Some(writer),
            }),
        };
        telemetry.record(TelemetryEvent::SessionStart {
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            terminal: std::env::var("TERM").unwrap_or_default(),
        });
        telemetry
    }

    /// Whether events are being collected (used by the status/about surfaces).
    pub fn is_enabled(&self) -> bool {
        self.inner.is_some()
    }

    /// Queues `event` for the writer thread. Never blocks: a full channel
    /// drops the event and bumps the counter reported in `SessionEnd`.
    pub fn record(&self, event: TelemetryEvent) {
        let Some(inner) = &self.inner else { return };
        let Some(tx) = &inner.tx else { return };
        if tx.try_send(event).is_err() {
            inner.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        // Best-effort flush: telemetry is a cache-grade side channel, so a
        // failed final send is deliberately ignored (per the error-handling
        // policy for best-effort caches).
        if let Some(tx) = inner.tx.take() {
            let _ = tx.try_send(TelemetryEvent::SessionEnd {
                duration_s: inner.started.elapsed().as_secs(),
                events_dropped: inner.dropped.load(Ordering::Relaxed),
            });
            drop(tx);
        }
        if let Some(writer) = inner.writer.take() {
            let _ = writer.join();
        }
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
