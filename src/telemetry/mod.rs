//! Opt-in, local-only usage telemetry (disabled by default).
//!
//! When `[telemetry] enabled = true` is set in `mantis.toml`, whitelisted
//! usage events are appended as JSON lines to a per-session, timestamped,
//! size-capped, rotated sink under `<state_dir>/telemetry/` (see
//! [`crate::session::state_dir`]).  Each session writes to its own file
//! `events-<session-epoch>.jsonl` so every file maps 1:1 to a mantis
//! session.  Nothing is ever sent anywhere: this module is the local-collection
//! groundwork for a future, separately-gated remote sink.  Events are a closed
//! enum ([`TelemetryEvent`]) so the schema is a whitelist by construction — no
//! paths, filenames, file content, or typed text can be recorded.  Raw
//! keystrokes are never captured; only resolved action ids are.  Recording is
//! non-blocking: [`Telemetry::record`] does a `try_send` onto a bounded
//! channel drained by a background writer thread, dropping events (counted,
//! reported in `SessionEnd`) rather than ever stalling the render loop.  When
//! disabled, the handle is a no-op: no thread is spawned and no files are
//! created.  Public items: [`Telemetry`], [`TelemetryEvent`], [`ActionSource`],
//! [`SessionSnapshot`].

mod sink;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
#[allow(clippy::large_enum_variant)]
pub enum TelemetryEvent {
    SessionStart {
        app_version: &'static str,
        os: &'static str,
        arch: &'static str,
        /// Value of `$TERM` (a terminal type like `xterm-256color`, not
        /// user data); empty when unset.
        terminal: String,
        // ---- Environment snapshot (same privacy rules as DiagnosticReport) ----
        os_version: Option<String>,
        wsl: bool,
        term_program: Option<String>,
        term_program_version: Option<String>,
        colorterm: Option<String>,
        windows_terminal: bool,
        ssh_session: bool,
        terminal_size: Option<(u16, u16)>,
        // ---- Workspace shape (counts only, no names or paths) ----
        tree_nodes: usize,
        tree_files: usize,
        tree_dirs: usize,
        tree_max_depth: usize,
        expanded_dirs: usize,
        tree_filter_active: bool,
        walk_errors: usize,
        git_repo: bool,
        git_mode: bool,
        // ---- Open file facts (extension only, never the name) ----
        file_open: bool,
        file_extension: Option<String>,
        file_size_bytes: Option<u64>,
        file_line_count: Option<usize>,
        file_encoding: Option<String>,
        file_line_ending: Option<String>,
        file_syntax: Option<String>,
        file_is_json: bool,
        file_is_diff: bool,
        file_uses_mmap: bool,
        // ---- Config overview ----
        theme: Option<String>,
        plugin_count: usize,
        telemetry_enabled: bool,
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

/// Snapshot of the environment and app state at session start, modelled on
/// [`crate::diagnostics::DiagnosticReport`] but without the bug-report body
/// or config-override paths.  Same privacy rules: counts, booleans, and
/// whitelisted env-var values only — never paths, filenames, file content, or
/// config values.
#[derive(Debug, Clone)]
pub struct SessionSnapshot {
    // Application / OS / terminal.
    pub app_version: &'static str,
    pub os: &'static str,
    pub arch: &'static str,
    pub terminal: String,
    pub os_version: Option<String>,
    pub wsl: bool,
    pub term_program: Option<String>,
    pub term_program_version: Option<String>,
    pub colorterm: Option<String>,
    pub windows_terminal: bool,
    pub ssh_session: bool,
    pub terminal_size: Option<(u16, u16)>,
    // Workspace shape.
    pub tree_nodes: usize,
    pub tree_files: usize,
    pub tree_dirs: usize,
    pub tree_max_depth: usize,
    pub expanded_dirs: usize,
    pub tree_filter_active: bool,
    pub walk_errors: usize,
    pub git_repo: bool,
    pub git_mode: bool,
    // Open file facts.
    pub file_open: bool,
    pub file_extension: Option<String>,
    pub file_size_bytes: Option<u64>,
    pub file_line_count: Option<usize>,
    pub file_encoding: Option<String>,
    pub file_line_ending: Option<String>,
    pub file_syntax: Option<String>,
    pub file_is_json: bool,
    pub file_is_diff: bool,
    pub file_uses_mmap: bool,
    // Config overview.
    pub theme: Option<String>,
    pub plugin_count: usize,
    pub telemetry_enabled: bool,
}

impl SessionSnapshot {
    pub fn collect(app: &crate::app::App) -> Self {
        let file_size_bytes = app
            .current_file
            .as_deref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len());
        SessionSnapshot {
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            terminal: std::env::var("TERM").unwrap_or_default(),
            os_version: os_version(),
            wsl: is_wsl(),
            term_program: whitelisted_env("TERM_PROGRAM"),
            term_program_version: whitelisted_env("TERM_PROGRAM_VERSION"),
            colorterm: whitelisted_env("COLORTERM"),
            windows_terminal: std::env::var_os("WT_SESSION").is_some(),
            ssh_session: std::env::var_os("SSH_CONNECTION").is_some(),
            terminal_size: crossterm::terminal::size().ok(),
            tree_nodes: app.nodes.len(),
            tree_files: app.nodes.iter().filter(|n| !n.is_dir).count(),
            tree_dirs: app.nodes.iter().filter(|n| n.is_dir).count(),
            tree_max_depth: app.nodes.iter().map(|n| n.depth).max().unwrap_or(0),
            expanded_dirs: app.expanded.len(),
            tree_filter_active: app.tree_filter.is_some(),
            walk_errors: app.walk_errors,
            git_repo: app.git_info.is_some(),
            git_mode: app.git_mode,
            file_open: app.current_file.is_some(),
            file_extension: app
                .current_file
                .as_deref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase()),
            file_size_bytes,
            file_line_count: app.current_file.is_some().then(|| app.line_count()),
            file_encoding: app.file_encoding.clone(),
            file_line_ending: app.file_line_ending.clone(),
            file_syntax: app.current_syntax.clone(),
            file_is_json: app.is_json,
            file_is_diff: app.is_diff,
            file_uses_mmap: app.virtual_file.is_some(),
            theme: app.config.theme.name.clone(),
            plugin_count: app.config.plugins.len(),
            telemetry_enabled: app.telemetry.is_enabled(),
        }
    }
}

fn whitelisted_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn os_version() -> Option<String> {
    if cfg!(target_os = "linux") {
        let raw = fs::read_to_string("/etc/os-release").ok()?;
        raw.lines()
            .find_map(|l| l.strip_prefix("PRETTY_NAME="))
            .map(|v| v.trim_matches('"').to_string())
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| format!("macOS {}", s.trim()))
    } else {
        None
    }
}

fn is_wsl() -> bool {
    cfg!(target_os = "linux")
        && fs::read_to_string("/proc/version").is_ok_and(|v| v.to_lowercase().contains("microsoft"))
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
    /// Does *not* emit a `SessionStart` event — that is done separately by the
    /// caller (typically [`App`](crate::app::App)) via
    /// [`record_session_start`](Self::record_session_start) after the app state
    /// is fully initialized.
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
    /// isolated directory).  Does *not* emit `SessionStart`; the caller is
    /// responsible for calling [`record_session_start`](Self::record_session_start)
    /// with a [`SessionSnapshot`] after initializing the app state.
    pub fn with_dir(dir: PathBuf) -> Self {
        let started = Instant::now();
        let id = TELEMETRY_ID_GEN.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::sync_channel::<TelemetryEvent>(CHANNEL_CAPACITY);
        let session_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let writer = std::thread::spawn(move || {
            let mut sink = JsonlSink::new(dir, session_epoch);
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

    /// Emits a `SessionStart` event populated from `snapshot`.  Call once
    /// after the app state is fully initialized (including the tree walk, open
    /// file, and plugin loading).
    pub fn record_session_start(&self, snapshot: SessionSnapshot) {
        self.record(TelemetryEvent::SessionStart {
            app_version: snapshot.app_version,
            os: snapshot.os,
            arch: snapshot.arch,
            terminal: snapshot.terminal,
            os_version: snapshot.os_version,
            wsl: snapshot.wsl,
            term_program: snapshot.term_program,
            term_program_version: snapshot.term_program_version,
            colorterm: snapshot.colorterm,
            windows_terminal: snapshot.windows_terminal,
            ssh_session: snapshot.ssh_session,
            terminal_size: snapshot.terminal_size,
            tree_nodes: snapshot.tree_nodes,
            tree_files: snapshot.tree_files,
            tree_dirs: snapshot.tree_dirs,
            tree_max_depth: snapshot.tree_max_depth,
            expanded_dirs: snapshot.expanded_dirs,
            tree_filter_active: snapshot.tree_filter_active,
            walk_errors: snapshot.walk_errors,
            git_repo: snapshot.git_repo,
            git_mode: snapshot.git_mode,
            file_open: snapshot.file_open,
            file_extension: snapshot.file_extension,
            file_size_bytes: snapshot.file_size_bytes,
            file_line_count: snapshot.file_line_count,
            file_encoding: snapshot.file_encoding,
            file_line_ending: snapshot.file_line_ending,
            file_syntax: snapshot.file_syntax,
            file_is_json: snapshot.file_is_json,
            file_is_diff: snapshot.file_is_diff,
            file_uses_mmap: snapshot.file_uses_mmap,
            theme: snapshot.theme,
            plugin_count: snapshot.plugin_count,
            telemetry_enabled: snapshot.telemetry_enabled,
        });
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
