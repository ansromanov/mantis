//! Central application state: the `App` struct that ties the whole TUI together.
//!
//! `App` holds the file tree, content/diff buffers, every overlay's state
//! (search, history, theme picker, symbol outline, command
//! palette, recent files, action menu, help, about, blame, goto line), provider
//! symbols and the
//! resolved theme and keymap, and the geometry captured during the last render
//! so mouse handlers can hit-test clicks. Construction (`App::new`) walks the
//! root, loads git status, and opens the first file; `reload`/`tick` keep the
//! view in sync with the filesystem via a debounced watcher with a periodic
//! fallback. Behaviour is split across sibling submodules (key/mouse handlers,
//! navigation, file_ops, loader, refresh, content/diff/fold helpers); this file
//! owns the struct, its fields, and a few shared free functions.
//!
//! The `DiffMode` enum governs which diff variant is shown in the content pane
//! when `is_diff` is `true`: `All` (default, `git diff HEAD`), `Staged`
//! (`git diff --cached`), or `Unstaged` (`git diff`). The active mode is cycled
//! with the `S` keybinding and is reflected in the content title badge.
//!
//! When `compare_base` is `Some(rev)`, the app is in compare mode: the tree
//! shows only files changed between `rev` and the working tree, and opening a
//! file shows `git diff <rev> -- <file>` instead of the usual working-tree diff.
//! Compare mode is entered via the command palette's `compare_against` action,
//! which opens a revision picker overlay (see `key_handlers/overlay.rs`) for
//! the target revision. Exiting git mode (Esc / toggle) clears `compare_base`
//! and returns to normal browsing.
//!
//! When `commit_base` is `Some(rev)`, the tree and content pane show the
//! selected commit's parent-to-commit changes, as chosen from the repository
//! commit log. This mode is also cleared when git mode is exited.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use notify::RecommendedWatcher;

use crate::config::{self, Config, Keymap};
use crate::fold::FoldRegion;
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::search::{
    BugReportState, CommandPalette, HistoryState, InFileSearch, RecentFilesState, RepoLogState,
    RevisionPicker, ThemePicker, TreeFilter, WorktreePicker,
};
use crate::selection::TextSelection;
use crate::theme::Theme;
use crate::tree::TreeNode;
use crate::virtual_file::VirtualFile;

mod applicability;
mod bug_report;
mod clipboard;
mod content_pos;
mod content_query;
mod context_menu;
mod diff_nav;
mod file_lists;
mod file_ops;
mod file_watch;
mod fold;
mod geometry;
mod init;
mod key_handlers;
mod lint;
mod loader;
mod markdown_view;
mod overlay;
pub use geometry::Geometry;
pub(crate) use overlay::Overlay;
pub(crate) mod menu_bar;
mod mouse_handlers;
mod mouse_overlays;
mod navigation;
mod pager;
mod refresh;
mod terminal;
mod types;
mod util;

use loader::Loader;

// Only referenced from test code (entry/state assertions).
#[cfg(test)]
use bug_report::resolve_bug_report_title;
#[cfg_attr(not(test), allow(unused_imports))]
pub use context_menu::{ContextActionId, ContextMenuEntry, ContextMenuState, ContextMenuTarget};
pub(crate) use terminal::{restore_terminal, set_alternate_scroll};
pub use types::{DiffMode, Focus, StatusMessage, TabAction};
pub(crate) use types::{HighlightCacheKey, HighlightCacheValue};
pub(crate) use util::{deleted_set, diff_line_style, rect_contains};

/// Central state struct for the `mantis` application. Handles the main event
/// loop, key/mouse dispatch, overlays, theme preset/color roles, git
/// status tracking, and file preview/diff rendering.
pub struct App {
    pub root: PathBuf,
    pub initial_root: PathBuf,
    pub nodes: Vec<TreeNode>,
    pub expanded: HashSet<PathBuf>,
    pub tree_selected: usize,
    /// Viewport top offset for the tree panel. The renderer always uses this
    /// as the first visible row; keyboard cursor movement calls
    /// `scroll_tree_into_view` to keep the selection visible.
    pub tree_scroll: usize,
    /// When `true`, PageUp/PageDown and Home/End scroll the tree viewport
    /// without moving the selection (cursor). Up/Down still move the cursor.
    pub tree_independent_scroll: bool,
    pub content: Vec<String>,
    pub highlighted: Vec<Vec<(ratatui::style::Style, String)>>,
    pub virtual_file: Option<VirtualFile>,
    pub is_json: bool,
    pub is_csv: bool,
    pub file_encoding: Option<String>,
    pub file_line_ending: Option<String>,
    pub show_pretty_json: bool,
    pub json_pretty_text: Vec<String>,
    pub json_pretty_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    /// JSON value path aligned with pretty-printed display lines.
    pub json_path_map: Vec<Option<String>>,
    pub show_csv_table: bool,
    pub csv_table_text: Vec<String>,
    pub csv_table_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    pub secret_original: Vec<String>,
    pub secret_masked: bool,
    pub secret_revealed: bool,
    pub content_scroll: usize,
    pub content_hscroll: usize,
    /// Pane width `content_scroll` was last expressed in (wrapped rows depend on
    /// it); 0 until the first draw. See `remap_scroll_for_width`.
    pub(crate) scroll_wrap_width: usize,
    /// Cursor (active line) index in the content pane (display-line coordinate).
    /// `j`/`k` move this when the content pane is focused; the viewport
    /// auto-scrolls to keep it visible.
    pub active_line: usize,
    /// When `true`, a one-line blame popup is shown for the active line.
    pub show_line_blame: bool,
    pub word_wrap: bool,
    pub current_file: Option<PathBuf>,
    /// Detected syntax/language name for the open file, or `None` for plain text
    /// or diffs. Set by `apply_file_load`; used by the status bar.
    pub current_syntax: Option<String>,
    pub is_diff: bool,
    /// Active diff variant: all changes vs HEAD, staged only, or unstaged only.
    pub diff_mode: DiffMode,
    /// When `true`, diffs render in a split old|new layout instead of unified.
    pub diff_side_by_side: bool,
    /// When `Some(short_hash)`, the content pane shows an immutable historical
    /// commit diff (from `show_selected_revision`/`H`). Guards `reload_content`
    /// from replacing it with the live working-tree diff. Cleared when the user
    /// returns to the live view (Esc, r, navigating to another file, etc.).
    pub viewing_revision: Option<String>,
    /// Full commit hash of `viewing_revision`. Used by the file-at-revision
    /// toggle to run `git show <hash>:<path>` and by Esc to restore the diff.
    pub viewing_revision_hash: Option<String>,
    /// When `Some(FileAtRevision)`, the content pane shows a read-only snapshot
    /// of the file as it was at a specific git revision. Set by the
    /// `toggle_file_revision` action; cleared by Esc, navigation, or reload.
    pub file_at_revision: Option<types::FileAtRevision>,
    /// Side-by-side rows parsed from the current diff; empty for non-diffs.
    pub diff_rows: Vec<crate::diff::DiffRow>,
    pub command_usage: crate::command_usage::UsageStats,
    pub content_title: Option<String>,
    pub focus: Focus,
    pub last_search_query: String,
    pub in_file_search: Option<InFileSearch>,
    /// Whether the in-file search bar currently owns keyboard input.
    pub in_file_search_open: bool,
    /// JSON query input overlay; active only for JSON and JSONL content.
    pub json_query: Option<String>,
    pub json_query_original: Vec<String>,
    /// Inline tree name filter, open when the user presses `/` with the tree
    /// focused. `None` means no filter is active; the full node list is shown.
    pub tree_filter: Option<TreeFilter>,
    pub command_palette: Option<CommandPalette>,
    pub history: Option<HistoryState>,
    /// State for the repo-wide commit log overlay. `Some` while the picker
    /// is open; `None` otherwise.
    pub repo_log: Option<RepoLogState>,
    pub theme_picker: Option<ThemePicker>,
    /// Persistent ring of recently opened file paths, most-recent-first, capped at
    /// `config.recent_files_count`. Maintained across overlay open/close cycles.
    pub recent_ring: Vec<PathBuf>,
    /// State for the recent-files overlay. `Some` while the picker is open.
    pub recent_files: Option<RecentFilesState>,
    /// Files pinned by the user for quick access within this workspace.
    pub bookmark_paths: Vec<PathBuf>,
    /// State for the bookmarks overlay. `Some` while the picker is open.
    pub bookmarks: Option<RecentFilesState>,
    /// State for the right-click context menu. `Some` while the menu is open.
    pub context_menu: Option<ContextMenuState>,
    /// Keyboard or pointer-opened action menu dropdown state.
    pub(crate) menu_bar_state: Option<menu_bar::MenuBarState>,
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    /// Monotonically increasing counter bumped every time the tree is rebuilt.
    /// Used to invalidate the tree-filter cache.
    pub tree_revision: u64,
    pub tree_width: u16,
    pub show_help: bool,
    pub help_scroll: crate::scroll::ScrollState,
    pub help_tab: usize,
    pub should_quit: bool,
    /// Set by the tab-related keybindings/palette entries, which this single-root `App` has
    /// no way to act on itself. The `Tabs` wrapper (`src/workspace.rs`) that
    /// owns the set of open tabs checks and clears this after every event.
    pub tab_action_request: Option<TabAction>,
    pub theme: Theme,
    pub git_status_enabled: bool,
    pub git_show_deleted: bool,
    pub git_show_untracked: bool,
    pub git_show_ignored: bool,
    pub git_info: Option<crate::git::GitRepoInfo>,
    /// Number of worktrees in the containing repository, used for discovery.
    pub worktree_count: usize,
    pub git_status_map: HashMap<PathBuf, GitStatus>,
    pub git_staged_files: HashSet<PathBuf>,
    pub git_unstaged_files: HashSet<PathBuf>,
    pub git_untracked_files: HashSet<PathBuf>,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    /// When `Some(rev)`, the tree and content pane show changes between `rev`
    /// and the working tree (compare mode). Set from the command palette (or
    /// the history overlay). Cleared when exiting git mode.
    pub compare_base: Option<String>,
    /// When set, git mode shows the selected commit's parent-to-commit diff.
    pub commit_base: Option<String>,
    /// State for the compare-against-revision picker overlay. `Some` while the
    /// picker is open; `None` otherwise.
    pub revision_picker: Option<RevisionPicker>,
    /// State for the git worktree switcher overlay.
    pub worktree_picker: Option<WorktreePicker>,
    pub bug_report: Option<BugReportState>,
    pub show_scrollbar: bool,
    pub show_scroll_percentage: bool,
    pub show_line_numbers: bool,
    pub show_blame: bool,
    /// Whether the full-file blame pane was visible before opening a blame
    /// commit snapshot, so Esc can restore the originating view.
    pub blame_before_commit: bool,
    pub show_about: bool,
    /// When `true`, the welcome overlay is shown on top of everything else.
    /// Set during construction on first-ever launch; cleared on dismiss and
    /// never set again (persisted via `welcome_shown.flag` in the state dir).
    pub show_welcome: bool,
    pub walk_errors: usize,
    /// Warning describing a malformed config that was ignored at startup, if any.
    pub config_error: Option<String>,
    /// Whether to automatically reload file content on disk change.
    pub auto_watch: bool,
    pub show_file_info: bool,
    pub indent_guides: bool,

    /// Whether to show file-type icon glyphs in the tree. The glyphs come from
    /// the static table in `crate::icons`; off by default because they
    /// require a Nerd Font in the terminal.
    pub icons_enabled: bool,
    keys: Keymap,
    pub config: Config,
    /// The config as last loaded from or written to `config_path`. Saves write
    /// only the differences from this snapshot, so settings the user never
    /// changed at runtime (and the file's comments) are left untouched.
    config_baseline: Config,
    config_path: Option<std::path::PathBuf>,
    /// The set of node indices (into `nodes`) that were visible during the last
    /// tree render. Populated by `draw_tree` when `tree_filter` is active so
    /// mouse handlers can map screen rows back to global indices.
    /// `None` means identity mapping (no filter active), avoiding allocations.
    pub tree_visible_indices: Option<Vec<usize>>,
    /// Per-node indent-guide masks, keyed by `tree_revision` so they're
    /// recomputed only when the tree is rebuilt rather than on every render.
    pub(crate) tree_guide_cache: Option<(u64, Vec<Vec<bool>>)>,
    /// Per-node affordance badges, keyed by `tree_revision`. Resolving one
    /// stats the path (and reads the directory, for directories), so entries
    /// are filled in lazily for the rows actually rendered and reused until
    /// the tree is rebuilt.
    pub(crate) tree_affordance_cache: Option<(u64, Vec<Option<crate::tree::TreeAffordance>>)>,
    /// Raw bytes + dimensions of the image in `current_file`, when it is an
    /// image and the terminal supports inline rendering. `None` otherwise. Set
    /// by the loader; consumed by `graphics::render_overlay` after each frame.
    pub content_image: Option<crate::graphics::ContentImage>,
    /// Clickable status-bar column ranges recorded during the last render.
    pub(crate) statusbar_segments: Vec<crate::ui::statusbar::StatusbarHit>,
    // Time and result index of the last search-result click, for double-click.
    last_click: Option<(Instant, usize)>,
    last_breadcrumb_click: Option<(Instant, std::path::PathBuf)>,
    // When the user last scrolled the content panel. The scrollbar overlay is
    // visible for 2 s after this instant. Initialised 10 s in the past so the
    // scrollbar is hidden on first render.
    pub content_scrolled_at: Instant,
    highlighter: Highlighter,
    last_refresh: Instant,
    file_watcher: Option<RecommendedWatcher>,
    file_watch_rx: Option<Receiver<notify::Result<notify::Event>>>,
    file_watch_path: Option<PathBuf>,
    /// Recursive watcher on the view root, used to drive tree/git refreshes from
    /// filesystem events instead of a blind timer. `None` if it could not be
    /// installed (e.g. the OS hit a watch-descriptor limit on a huge tree), in
    /// which case `tick` falls back to the periodic reload.
    root_watcher: Option<RecommendedWatcher>,
    root_watch_rx: Option<Receiver<notify::Result<notify::Event>>>,
    /// Watcher on the config file (`mantis.toml`) so that edits to keybindings,
    /// theme, or other settings are detected and hot-reloaded (or a restart
    /// hint is shown). `None` when no config path was provided.
    #[allow(dead_code)]
    config_watcher: Option<RecommendedWatcher>,
    config_watch_rx: Option<Receiver<notify::Result<notify::Event>>>,
    /// A relevant config-file event was seen and a debounced reload is due.
    config_dirty: bool,
    /// Instant of the most recent config-watch event, used to debounce bursts
    /// (e.g. an editor's atomic temp-write-then-rename save) into a single
    /// reload once the config file goes quiet, mirroring `tree_dirty_at`.
    config_dirty_at: Option<Instant>,
    /// A relevant root filesystem event was seen and a debounced reload is due.
    tree_dirty: bool,
    /// Instant of the most recent root event, used to debounce bursts (e.g. a
    /// build touching many files) into a single reload once the tree goes quiet.
    tree_dirty_at: Option<Instant>,
    pub selection: Option<TextSelection>,
    drag_start: Option<(usize, usize)>,
    scrollbar_drag: bool,
    splitter_drag: bool,
    /// Set to `true` after suspending the TUI (e.g. for editor), signals
    /// `main.rs` to call `terminal.clear()` before the next `draw()`.
    pub needs_clear: bool,
    // Folding state (built-in YAML detector or language provider)
    pub fold_regions: Vec<FoldRegion>,
    pub folded: HashSet<usize>,
    /// Per-path symbol outlines supplied by language providers via `set_symbols`.
    pub symbols: HashMap<PathBuf, Vec<crate::code_intel::Symbol>>,
    /// Per-path diagnostics returned by the selected language provider.
    pub file_diagnostics: HashMap<PathBuf, Vec<crate::code_intel::Diagnostic>>,
    /// Highest-severity diagnostic per physical line, precomputed for visible-row rendering.
    pub file_diagnostic_lines:
        HashMap<PathBuf, HashMap<usize, crate::code_intel::DiagnosticSeverity>>,
    /// Sender cloned into each background lint thread.
    pub(crate) diagnostics_tx: std::sync::mpsc::Sender<lint::LintResult>,
    /// Receives finished lint runs; drained by `tick`.
    pub(crate) diagnostics_rx: std::sync::mpsc::Receiver<lint::LintResult>,
    /// Monotonic id for lint requests.
    pub(crate) diagnostic_seq: u64,
    /// Latest outstanding diagnostic request for each path; older replies are ignored.
    pub(crate) latest_diagnostic_requests: HashMap<PathBuf, u64>,
    /// display_line → physical_line mapping; empty when no folds are active.
    pub fold_display_map: Vec<usize>,
    /// (screen_y, region_idx) pairs recorded during the last render, used for
    /// fold-gutter mouse click detection.
    pub fold_gutter_rows: Vec<(u16, usize)>,
    /// YAML parse error message, if any; only set for `.yaml`/`.yml` files.
    pub yaml_error: Option<String>,
    /// Number of YAML anchors (`&name`) found in the current file.
    pub yaml_anchor_count: usize,
    /// Kubernetes manifest summary for the open YAML file, shown in the status bar.
    pub k8s_summary: Option<crate::k8s_facts::K8sSummary>,
    /// Number of YAML aliases (`*name`) found in the current file.
    pub yaml_alias_count: usize,
    pub is_log: bool,
    pub is_jsonl: bool,
    pub(crate) jsonl_source: Vec<String>,
    pub(crate) jsonl_expanded: HashSet<usize>,
    pub(crate) jsonl_display_map: Vec<usize>,
    pub follow_mode: bool,
    pub follow_pinned: bool,
    /// Most recent qualifying filesystem event waiting for follow-mode handling.
    pub(crate) follow_candidate: Option<PathBuf>,
    /// User interaction temporarily pauses automatic file switching.
    pub(crate) follow_paused_until: Option<Instant>,
    pub(crate) log_highlight_cache:
        std::cell::RefCell<std::collections::HashMap<usize, Vec<(ratatui::style::Style, String)>>>,
    pub content_revision: u64,
    pub filter_query: Option<String>,
    pub filter_display_map: Vec<usize>,
    pub(crate) filter_cache:
        std::cell::RefCell<std::collections::HashMap<(u64, String), Vec<usize>>>,
    pub filter_bar: Option<crate::search::FilterBarState>,
    /// Background worker that reads/highlights files and runs git diffs off the
    /// main thread so tree navigation never blocks the event loop.
    loader: Loader,
    /// Sequence number of the most recently dispatched load. Worker responses
    /// tagged with an older `seq` are stale (superseded by a newer navigation)
    /// and discarded.
    load_seq: u64,
    /// Whether a background load is currently in flight; drives the "loading…"
    /// indicator in the content title.
    pub loading: bool,
    /// Sequence number for the most recently dispatched git-status request.
    /// Responses with a lower seq are stale and discarded, which coalesces
    /// rapid root changes / toggles into one scan.
    git_seq: u64,
    /// Rendered content keyed by file path. Populated by the markdown
    /// view; the content pane checks this before markdown/virtual-file rendering.
    pub rendered_content: HashMap<PathBuf, Vec<Vec<(ratatui::style::Style, String)>>>,
    /// Plain text of the rendered content, keyed by file path (same keys as
    /// `rendered_content`). Used for in-file search, selection, and blame display.
    /// Stored separately because `rendered_content` holds styled spans whose text
    /// would require joining on every access.
    pub rendered_text: HashMap<PathBuf, Vec<String>>,
    /// Remembered cursor position per file (active_line, content_scroll), so
    /// returning to a previously-viewed file restores where you left off. Session-
    /// scoped (in memory); restart restore of the last file is handled by session.
    pub cursor_positions: HashMap<PathBuf, (usize, usize)>,
    /// Set to `true` when rendered content is installed for the current file and
    /// reset to `false` when `current_file` changes.
    pub rendered_active: bool,
    /// When `true`, rendered markdown files are instead displayed as raw plain text natively.
    pub show_raw_markdown: bool,
    /// The last path for which rendered content was installed while it was the
    /// current file. Used to detect re-renders of the same file vs. first-time
    /// renders of a newly opened file, so scroll position is preserved across
    /// re-renders.
    pub(super) rendered_active_path: Option<PathBuf>,
    /// Transient status message (e.g. "path copied"), shown until the next keypress
    /// or auto-expires after ~3 seconds.
    pub status_message: Option<StatusMessage>,
    /// Cache of the most recently highlighted visible window, so consecutive
    /// renders with the same scroll/theme/content skip re-highlighting.
    pub(crate) content_highlight_cache: RefCell<Option<(HighlightCacheKey, HighlightCacheValue)>>,
    /// When `true`, the session cache needs to be re-written.
    session_dirty: bool,
    /// When the session was last dirtied, for debounced writes.
    session_dirty_at: Option<std::time::Instant>,
    /// Last time the session cache was flushed to disk.
    session_last_save: std::time::Instant,
    /// Texts captured by `copy_to_clipboard` under test, in call order. Tests
    /// assert against this instead of the real system clipboard.
    #[cfg(test)]
    pub(crate) clipboard_capture: Vec<String>,
    /// Whether a newer version is available.
    pub new_version_available: Option<String>,
    /// Background channel receiver for update check results.
    pub(crate) update_rx: Option<std::sync::mpsc::Receiver<String>>,
    /// Panel/popup rectangles and list offsets recorded by the renderer.
    pub geo: Geometry,
}

impl App {
    /// Persists the current config to disk if a config path was provided.
    /// Surfaces a status message on failure so the user isn't silently
    /// reverted on next launch.
    fn save_config(&mut self) {
        if let Some(path) = &self.config_path {
            let result = config::save_changes(&self.config_baseline, &self.config, path);
            if result.is_ok() {
                self.config_baseline = self.config.clone();
            }
            if let Err(e) = result {
                self.set_status(format!("could not save config: {e}"));
            }
        }
    }

    /// Marks the session cache as needing a write. The actual write is
    /// debounced in `tick` so rapid state changes (scrolling, repeated
    /// expand/collapse) coalesce into one disk write.
    pub(crate) fn mark_session_dirty(&mut self) {
        self.session_dirty = true;
        self.session_dirty_at = Some(self.now());
    }

    /// Immediately writes the current session state to the cache. Called on
    /// quit and by the debounced `tick` path.
    pub(crate) fn save_session(&mut self) {
        let state = crate::session::SessionState {
            expanded: self.expanded.iter().cloned().collect(),
            bookmarks: self.bookmark_paths.clone(),
            current_file: self.current_file.clone(),
            content_scroll: self.content_scroll,
            active_line: self.active_line,
            initial_root: Some(self.initial_root.clone()),
        };
        crate::session::save(&self.root, &state);
        self.session_dirty = false;
        self.session_dirty_at = None;
        self.session_last_save = self.now();
    }

    /// Rebuilds the file tree, re-fetches git status (async), and reloads the
    /// current file. Triggered explicitly by the reload key, by debounced
    /// filesystem events from the root watcher, or by the periodic fallback
    /// timer when no root watcher is installed.
    pub fn reload(&mut self) {
        self.reload_tree();
        self.reload_content();
    }

    /// Like [`reload`](Self::reload) but leaves the open file alone. Used for
    /// filesystem-watcher driven refreshes: the per-file watcher already
    /// reloads the open file when *it* changes, so re-reading and
    /// re-highlighting it for unrelated events is wasted work.
    pub(crate) fn reload_tree(&mut self) {
        self.last_refresh = self.now();
        if self.git_status_enabled {
            if let Some(ref rev) = self.compare_base.clone() {
                self.request_range_status(rev.clone());
            } else if let Some(ref rev) = self.commit_base.clone() {
                match crate::git::commit_status(&self.root, rev) {
                    Ok(map) => {
                        self.git_status_map = map;
                        self.expand_git_dirs();
                        self.rebuild(false);
                        self.try_open_selected();
                    }
                    Err(error) => self.set_status(format!("git commit status failed: {error}")),
                }
            } else {
                self.request_git_status_refresh();
            }
        }
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        let changed = self.git_changed_files_set();
        if let Some(s) = self
            .command_palette
            .as_mut()
            .and_then(|p| p.route_search.as_mut())
        {
            s.reload_files(&root, show_hidden, ignore_gitignore, changed.as_ref());
        }
        self.rebuild(false);
    }

    /// Injectible time source. Returns `Instant::now()` in production; tests may
    /// substitute a mock to avoid wall-clock waits.
    pub(crate) fn now(&self) -> Instant {
        Instant::now()
    }

    /// Sets a transient status message with an auto-expiry timestamp.
    pub fn set_status(&mut self, msg: impl Into<String>) {
        let sm = StatusMessage::new(msg, self.now());
        self.status_message = Some(sm);
    }

    /// Returns the set of changed file paths (not directories) when in git mode,
    /// or `None` when git mode is inactive. Used to scope the search index to
    /// changed files only.
    pub(crate) fn git_changed_files_set(&self) -> Option<HashSet<PathBuf>> {
        if !self.git_mode {
            return None;
        }
        let files: HashSet<PathBuf> = self
            .git_status_map
            .iter()
            .filter(|(path, _)| path.starts_with(&self.root) && !path.is_dir())
            .map(|(path, _)| path.clone())
            .collect();
        Some(files)
    }

    /// Records that the user scrolled the content, used to show a transient
    /// scrollbar.
    pub fn mark_content_scrolled(&mut self) {
        self.content_scrolled_at = self.now();
    }

    pub fn keys(&self) -> &Keymap {
        &self.keys
    }

    /// Mutable keymap access for tests outside the `app` module.
    #[cfg(test)]
    pub(crate) fn keys_mut_for_tests(&mut self) -> &mut Keymap {
        &mut self.keys
    }

    /// Human-readable label for the tree panel's current view mode, shown as the
    /// panel's border title. Extends as new tree modes are added (functions,
    /// file structure, …); today it reflects only Files vs Git.
    pub fn panel_mode_label(&self) -> &'static str {
        if self.git_mode {
            if self.git_mode_flat {
                "Git · flat"
            } else {
                "Git"
            }
        } else {
            "Files"
        }
    }

    /// Clears all content-pane state (file buffer, diff, highlights, rendered
    /// JSON, scroll, selection, fold state, rendered content, etc.).
    /// Called when the current file is no longer valid — git mode becomes clean
    /// (no changed files) or the viewer root changes. Does NOT clear
    /// file watchers (those are managed at the call site).
    fn clear_content_state(&mut self) {
        self.current_file = None;
        self.current_syntax = None;
        self.content = Vec::new();
        self.content_image = None;
        self.highlighted = Vec::new();
        self.virtual_file = None;
        self.is_json = false;
        self.is_jsonl = false;
        self.jsonl_source.clear();
        self.jsonl_expanded.clear();
        self.jsonl_display_map.clear();
        self.file_encoding = None;
        self.file_line_ending = None;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.secret_original.clear();
        self.secret_masked = false;
        self.secret_revealed = false;
        self.viewing_revision = None;
        self.viewing_revision_hash = None;
        self.file_at_revision = None;
        self.set_content_scroll(0);
        self.content_hscroll = 0;
        self.active_line = 0;
        self.show_line_blame = false;
        self.is_diff = false;
        self.diff_side_by_side = false;
        self.diff_rows = Vec::new();
        self.content_title = None;
        self.selection = None;
        self.drag_start = None;
        self.fold_regions = Vec::new();
        self.folded = HashSet::new();
        self.fold_display_map = Vec::new();
        self.yaml_error = None;
        self.yaml_anchor_count = 0;
        self.yaml_alias_count = 0;
        self.in_file_search = None;
        self.in_file_search_open = false;
        self.json_query = None;
        self.json_query_original.clear();
        self.rendered_active = false;
        self.rendered_content.clear();
        self.rendered_text.clear();
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
