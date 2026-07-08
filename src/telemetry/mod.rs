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
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use serde::Serialize;

use sink::JsonlSink;

/// How an action was invoked.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionSource {
    Palette,
    Key,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayKind {
    Help,
    About,
    ThemePicker,
    PluginPicker,
    CommandPalette,
    History,
    RecentFiles,
    Search,
    InFileSearch,
    TreeFilter,
    BugReport,
    CompareInput,
    GotoLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Feature {
    Fold,
    DiffNav,
    GitHistory,
    VisualMode,
    GitBlame,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSourceKind {
    Tree,
    RecentFiles,
    Search,
    History,
    Startup,
    Reopen,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileSizeBucket {
    Under1Kb,
    From1KbTo1Mb,
    From1MbTo16Mb,
    Over16Mb,
}

impl FileSizeBucket {
    pub fn from_size(size: u64) -> Self {
        if size < 1024 {
            Self::Under1Kb
        } else if size < 1024 * 1024 {
            Self::From1KbTo1Mb
        } else if size < 16 * 1024 * 1024 {
            Self::From1MbTo16Mb
        } else {
            Self::Over16Mb
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileEncoding {
    Utf8,
    Ascii,
    Utf8Bom,
    Binary,
    Unknown,
}

impl FileEncoding {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(enc: Option<&str>) -> Self {
        match enc {
            Some("UTF-8") => Self::Utf8,
            Some("ASCII") => Self::Ascii,
            Some("UTF-8 BOM") => Self::Utf8Bom,
            Some("BINARY") => Self::Binary,
            _ => Self::Unknown,
        }
    }
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
    OverlayOpened {
        kind: OverlayKind,
    },
    FeatureUsed {
        feature: Feature,
    },
    PluginToggled {
        kind: crate::plugin::types::PluginKind,
        enabled: bool,
    },
    FileOpened {
        size_bucket: FileSizeBucket,
        source_kind: FileSourceKind,
        encoding: FileEncoding,
        is_binary: bool,
    },
    PerfSpan {
        span: &'static str,
        duration_bucket: &'static str,
    },
    ErrorOccurred {
        module: &'static str,
        kind: &'static str,
    },
}

/// Bounded queue between the render loop and the writer thread. Sized so a
/// burst of palette commands never blocks; overflow drops events instead.
const CHANNEL_CAPACITY: usize = 256;

struct TelemetryState {
    id: u64,
    tx: SyncSender<TelemetryEvent>,
    dropped: Arc<AtomicU64>,
}

static TELEMETRY_STATE: Mutex<Option<TelemetryState>> = Mutex::new(None);
static TELEMETRY_ID_GEN: AtomicU64 = AtomicU64::new(0);

/// Telemetry layer for the tracing library.
pub struct TelemetryLayer;

struct SpanStart(Instant);

impl<S> tracing_subscriber::Layer<S> for TelemetryLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_new_span(
        &self,
        _attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if let Some(span) = ctx.span(id) {
            const ALLOWED_SPANS: &[&str] = &[
                "open_file",
                "build_visible",
                "highlight",
                "highlight_range",
                "search_refresh",
                "diff_parse",
                "plugin_round_trip",
            ];
            if ALLOWED_SPANS.contains(&span.name()) {
                span.extensions_mut().insert(SpanStart(Instant::now()));
            }
        }
    }

    fn on_close(&self, id: tracing::span::Id, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(span) = ctx.span(&id) {
            let name = span.name();
            const ALLOWED_SPANS: &[&str] = &[
                "open_file",
                "build_visible",
                "highlight",
                "highlight_range",
                "search_refresh",
                "diff_parse",
                "plugin_round_trip",
            ];
            if !ALLOWED_SPANS.contains(&name) {
                return;
            }
            let start = span.extensions().get::<SpanStart>().map(|s| s.0);
            if let Some(start) = start {
                let duration = start.elapsed();
                let duration_ms = duration.as_millis() as u64;
                let bucket = if duration_ms < 1 {
                    "<1ms"
                } else if duration_ms < 16 {
                    "1-16ms"
                } else if duration_ms < 100 {
                    "16-100ms"
                } else {
                    ">100ms"
                };
                record_global(TelemetryEvent::PerfSpan {
                    span: name,
                    duration_bucket: bucket,
                });
            }
        }
    }
}

pub(crate) fn record_global(event: TelemetryEvent) {
    if let Ok(guard) = TELEMETRY_STATE.lock() {
        if let Some(state) = &*guard {
            if state.tx.try_send(event).is_err() {
                state.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}

/// Handle owned by `App`. Cheap to call from anywhere; all I/O happens on the
/// background writer thread. Dropping the handle emits `SessionEnd` and joins
/// the writer so buffered events are flushed on every exit path.
pub struct Telemetry {
    inner: Option<Inner>,
}

struct Inner {
    id: u64,
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
        let id = TELEMETRY_ID_GEN.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::sync_channel::<TelemetryEvent>(CHANNEL_CAPACITY);
        let writer = std::thread::spawn(move || {
            let mut sink = JsonlSink::new(dir);
            while let Ok(event) = rx.recv() {
                sink.append(&event, started.elapsed());
            }
        });
        let telemetry = Telemetry {
            inner: Some(Inner {
                id,
                tx: Some(tx.clone()),
                dropped: Arc::new(AtomicU64::new(0)),
                started,
                writer: Some(writer),
            }),
        };
        if let Ok(mut guard) = TELEMETRY_STATE.lock() {
            *guard = Some(TelemetryState {
                id,
                tx,
                dropped: telemetry.inner.as_ref().unwrap().dropped.clone(),
            });
        }
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
        record_global(event);
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        let Some(inner) = &mut self.inner else { return };
        if let Ok(mut guard) = TELEMETRY_STATE.lock() {
            if let Some(state) = &*guard {
                if state.id == inner.id {
                    *guard = None;
                }
            }
        }
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
