//! Central application state: the `App` struct that ties the whole TUI together.
//!
//! `App` holds the file tree, content/diff buffers, every overlay's state
//! (search, history, theme picker, plugin picker, command palette, recent files,
//! help, about, blame, goto line), the
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
//! which opens a prompt (`compare_input`, see `key_handlers/overlay.rs`) for
//! the target revision. Exiting git mode (Esc / toggle) clears `compare_base`
//! and returns to normal browsing.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::fold::FoldRegion;
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::plugin::{ExtraSyntax, PluginContributions, PluginManager};
use crate::search::{
    BugReportState, CommandPalette, CompareModeInput, GotoLineState, HistoryState, InFileSearch,
    PluginPicker, RecentFilesState, SearchState, ThemePicker, TreeFilter,
};
use crate::selection::TextSelection;
use crate::theme::Theme;
use crate::tree::TreeNode;
use crate::virtual_file::VirtualFile;

mod content_pos;
mod content_query;
mod diff_nav;
mod file_ops;
mod fold;
mod init;
mod key_handlers;
mod loader;
mod mouse_handlers;
mod navigation;
mod pager;
mod plugin_ops;
mod refresh;
mod types;
mod util;

use loader::Loader;

pub use types::{DiffMode, Focus, StatusMessage};
pub(crate) use types::{HighlightCacheKey, HighlightCacheValue, PendingKeypress};
pub(crate) use util::{deleted_set, diff_line_style, rect_contains};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveOverlays {
    pub help: bool,
    pub about: bool,
    pub theme_picker: bool,
    pub plugin_picker: bool,
    pub command_palette: bool,
    pub history: bool,
    pub recent_files: bool,
    pub search: bool,
    pub in_file_search: bool,
    pub tree_filter: bool,
    pub bug_report: bool,
    pub compare_input: bool,
    pub goto_line: bool,
    pub visual_mode: bool,
    pub git_blame: bool,
}

/// Central state struct for the `mantis` application. Handles the main event
/// loop, key/mouse dispatch, overlays, plugins, theme preset/color roles, git
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
    pub file_encoding: Option<String>,
    pub file_line_ending: Option<String>,
    pub show_pretty_json: bool,
    pub json_pretty_text: Vec<String>,
    pub json_pretty_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    pub content_scroll: usize,
    pub content_hscroll: usize,
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
    /// Side-by-side rows parsed from the current diff; empty for non-diffs.
    pub diff_rows: Vec<crate::diff::DiffRow>,
    pub command_usage: crate::command_usage::UsageStats,
    /// Opt-in local telemetry handle; a no-op when `[telemetry]` is disabled.
    /// Dropping it (with `App`) flushes and joins the writer thread.
    pub telemetry: crate::telemetry::Telemetry,
    pub(crate) last_open_source: crate::telemetry::FileSourceKind,
    pub(crate) active_overlays: ActiveOverlays,
    pub content_title: Option<String>,
    pub focus: Focus,
    pub search: Option<SearchState>,
    pub last_search_query: String,
    pub in_file_search: Option<InFileSearch>,
    /// Inline tree name filter, open when the user presses `/` with the tree
    /// focused. `None` means no filter is active; the full node list is shown.
    pub tree_filter: Option<TreeFilter>,
    /// Inline go-to-line dialog, open when the user presses the `goto_line`
    /// keybinding (default `:`). `Some` while the dialog is open.
    pub goto_line: Option<GotoLineState>,
    pub command_palette: Option<CommandPalette>,
    pub history: Option<HistoryState>,
    pub theme_picker: Option<ThemePicker>,
    /// State for the plugin manager overlay. `Some` while the picker is open.
    pub plugin_picker: Option<PluginPicker>,
    /// Hit area of the plugin list recorded during the last render.
    pub plugin_picker_area: Rect,
    /// Scroll offset of the plugin list recorded during the last render.
    pub plugin_picker_offset: usize,
    /// Persistent ring of recently opened file paths, most-recent-first, capped at
    /// `config.recent_files_count`. Maintained across overlay open/close cycles.
    pub recent_ring: Vec<PathBuf>,
    /// State for the recent-files overlay. `Some` while the picker is open.
    pub recent_files: Option<RecentFilesState>,
    /// Hit area of the recent-files list recorded during the last render.
    pub recent_area: Rect,
    /// Scroll offset of the recent-files list recorded during the last render.
    pub recent_offset: usize,
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    /// Monotonically increasing counter bumped every time the tree is rebuilt.
    /// Used to invalidate the tree-filter cache.
    pub tree_revision: u64,
    pub tree_width: u16,
    pub show_help: bool,
    pub help_scroll: crate::scroll::ScrollState,
    pub help_tab: usize,
    pub help_area: ratatui::layout::Rect,
    pub bug_report_area: ratatui::layout::Rect,
    pub bug_report_preview_area: ratatui::layout::Rect,
    pub should_quit: bool,
    pub theme: Theme,
    pub git_status_enabled: bool,
    pub git_show_deleted: bool,
    pub git_show_untracked: bool,
    pub git_show_ignored: bool,
    pub git_info: Option<crate::git::GitRepoInfo>,
    pub git_status_map: HashMap<PathBuf, GitStatus>,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    /// When `Some(rev)`, the tree and content pane show changes between `rev`
    /// and the working tree (compare mode). Set from the command palette (or
    /// the history overlay). Cleared when exiting git mode.
    pub compare_base: Option<String>,
    /// State for the compare-against-revision input prompt. `Some` while the
    /// prompt is open (user typing a revision); `None` otherwise.
    pub compare_input: Option<CompareModeInput>,
    pub bug_report: Option<BugReportState>,
    pub show_scrollbar: bool,
    pub show_scroll_percentage: bool,
    pub show_line_numbers: bool,
    pub show_blame: bool,
    pub show_about: bool,
    pub show_telemetry_notice: bool,
    pub walk_errors: usize,
    /// Warning describing a malformed config that was ignored at startup, if any.
    pub config_error: Option<String>,
    /// Whether to automatically reload file content on disk change.
    pub auto_watch: bool,
    pub show_file_info: bool,
    pub indent_guides: bool,
    /// Scroll offset for the blame pane (first visible line). Synced with
    /// `content_scroll` so the cursor stays in view.
    pub blame_scroll: usize,
    /// Whether to show file-type icon glyphs in the tree. The glyphs come from
    /// a plugin via the `set_icon_map` action; off by default because they
    /// require a Nerd Font in the terminal.
    pub icons_enabled: bool,
    /// Extension (lowercase) → Nerd Font icon glyph, provided by the iconize
    /// plugin. Used when `icons_enabled` is true and non-empty.
    pub icon_map: HashMap<String, String>,
    /// Folder icon for expanded directories.
    pub icon_dir_open: String,
    /// Folder icon for collapsed directories.
    pub icon_dir_closed: String,
    /// Fallback icon for file types not in `icon_map`.
    pub icon_fallback: String,
    keys: Keymap,
    pub config: Config,
    config_path: Option<std::path::PathBuf>,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
    /// The set of node indices (into `nodes`) that were visible during the last
    /// tree render. Populated by `draw_tree` when `tree_filter` is active so
    /// mouse handlers can map screen rows back to global indices.
    /// `None` means identity mapping (no filter active), avoiding allocations.
    pub tree_visible_indices: Option<Vec<usize>>,
    /// Per-node indent-guide masks, keyed by `tree_revision` so they're
    /// recomputed only when the tree is rebuilt rather than on every render.
    pub(crate) tree_guide_cache: Option<(u64, Vec<Vec<bool>>)>,
    pub content_area: Rect,
    pub search_area: Rect,
    pub search_offset: usize,
    pub command_palette_area: Rect,
    pub command_palette_offset: usize,
    pub history_area: Rect,
    pub history_offset: usize,
    pub theme_area: Rect,
    pub theme_offset: usize,
    /// Hit area of the splitter bar between tree and content panes.
    pub splitter_area: Rect,
    // Time and result index of the last search-result click, for double-click.
    last_click: Option<(Instant, usize)>,
    last_breadcrumb_click: Option<(Instant, std::path::PathBuf)>,
    // When the user last scrolled the content panel. The scrollbar overlay is
    // visible for 2 s after this instant. Initialised 10 s in the past so the
    // scrollbar is hidden on first render.
    pub content_scrolled_at: Instant,
    highlighter: Highlighter,
    /// Syntax definitions loaded from plugins. Kept so `apply_theme` can
    /// rebuild the highlighter without losing plugin syntax definitions.
    extra_syntaxes: Vec<ExtraSyntax>,
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
    /// Per-path fold regions supplied by language providers via `set_fold_regions`.
    /// When the named path is the current file, regions are applied immediately.
    pub plugin_fold_regions: HashMap<PathBuf, Vec<FoldRegion>>,
    /// display_line → physical_line mapping; empty when no folds are active.
    pub fold_display_map: Vec<usize>,
    /// (screen_y, region_idx) pairs recorded during the last render, used for
    /// fold-gutter mouse click detection.
    pub fold_gutter_rows: Vec<(u16, usize)>,
    /// YAML parse error message, if any; only set for `.yaml`/`.yml` files.
    pub yaml_error: Option<String>,
    /// Number of YAML anchors (`&name`) found in the current file.
    pub yaml_anchor_count: usize,
    /// Number of YAML aliases (`*name`) found in the current file.
    pub yaml_alias_count: usize,
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
    /// Plugin subprocess manager. Spawned at startup, deactivated on quit.
    pub(crate) plugin_manager: PluginManager,
    /// Set to `true` while handling a plugin-originated `open_file` action, so
    /// `apply_file_load` / `apply_diff_load` can suppress the re-emission of
    /// `on_file_open` back to plugins (breaking the recursion loop).
    plugin_is_opening_file: bool,
    /// Most recent plugin message, shown in the status bar.
    pub plugin_message: Option<String>,
    /// Most recent `plugin_error` action (protocol 3+), shown in the status
    /// bar with error styling. Distinct from `plugin_message`, which is for
    /// routine `show_message` text.
    pub plugin_error: Option<String>,
    /// A keypress dispatched to `on_keypress` subscribers, awaiting a
    /// `key_handled` reply before its deadline (protocol 3+). `None` when no
    /// keypress is currently in flight. See `key_handlers::mod` for where
    /// this is set and `refresh::process_pending_keypress` for where it is
    /// resolved.
    pub(crate) pending_keypress: Option<PendingKeypress>,
    /// Set to `true` by the `key_handled` action handler when a reply arrives
    /// for the current `pending_keypress`. Consumed and reset by
    /// `process_pending_keypress` on the next tick.
    pub(crate) pending_keypress_handled: bool,
    /// Tracks what application state each plugin has contributed so that
    /// disabling or crashing the plugin tears down exactly its output.
    /// Populated by `handle_plugin_action` and consumed by
    /// `teardown_plugin_contributions`.
    pub(crate) plugin_contributions: HashMap<String, PluginContributions>,
    /// Plugin-rendered content keyed by file path. Populated by the `set_content`
    /// action; the content pane checks this before markdown/virtual-file rendering.
    pub plugin_content: HashMap<PathBuf, Vec<Vec<(ratatui::style::Style, String)>>>,
    /// Plain text of plugin-provided content, keyed by file path (same keys as
    /// `plugin_content`). Used for in-file search, selection, and blame display.
    /// Stored separately because `plugin_content` holds styled spans whose text
    /// would require joining on every access.
    pub plugin_content_text: HashMap<PathBuf, Vec<String>>,
    /// Remembered cursor position per file (active_line, content_scroll), so
    /// returning to a previously-viewed file restores where you left off. Session-
    /// scoped (in memory); restart restore of the last file is handled by session.
    pub cursor_positions: HashMap<PathBuf, (usize, usize)>,
    /// Set to `true` when a plugin sends `set_content` for the current file and
    /// reset to `false` when `current_file` changes, so the `[rendering…]`
    /// placeholder only shows while the plugin is actively working on that file.
    pub plugin_content_active: bool,
    /// The last path for which a plugin sent `set_content` while it was the
    /// current file. Used to detect re-renders of the same file vs. first-time
    /// renders of a newly opened file, so scroll position is preserved across
    /// plugin re-render ticks.
    pub(super) plugin_content_active_path: Option<PathBuf>,
    /// Transient status message (e.g. "path copied"), shown until the next keypress
    /// or auto-expires after ~3 seconds.
    pub status_message: Option<StatusMessage>,
    /// Breadcrumb segment areas recorded during the last render, used for mouse
    /// hit-testing. Each entry is (target_directory_path, clickable_rect).
    pub breadcrumb_areas: Vec<(std::path::PathBuf, Rect)>,
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
}

impl App {
    /// Persists the current config to disk if a config path was provided.
    /// Surfaces a status message on failure so the user isn't silently
    /// reverted on next launch.
    fn save_config(&mut self) {
        if let Some(path) = &self.config_path {
            if let Err(e) = config::save(&self.config, path) {
                self.telemetry
                    .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                        module: "config",
                        kind: "save_failed",
                    });
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

    /// Tears down all application state produced by the named plugin.
    ///
    /// Removes content, fold-region, and icon-map contributions, clears the
    /// plugin's provider registrations, and reloads the current file if the
    /// plugin had rendered content for it — so the display falls back to core
    /// rendering (markdown, JSON, or plain text).
    pub(crate) fn teardown_plugin_contributions(&mut self, name: &str) {
        let Some(contrib) = self.plugin_contributions.remove(name) else {
            return;
        };

        // Content — clear plugin-rendered lines for contributed paths.
        for path in &contrib.content_paths {
            self.plugin_content.remove(path);
            self.plugin_content_text.remove(path);
        }
        let had_current_content = contrib
            .content_paths
            .iter()
            .any(|p| self.current_file.as_deref() == Some(p));

        // Fold regions.
        for path in &contrib.fold_region_paths {
            self.plugin_fold_regions.remove(path);
        }
        if contrib
            .fold_region_paths
            .iter()
            .any(|p| self.current_file.as_deref() == Some(p))
        {
            self.clear_fold_state();
        }

        // Icon map (Nerd Font glyphs).
        if contrib.has_icon_map {
            self.icons_enabled = false;
            self.icon_map.clear();
            self.icon_dir_open.clear();
            self.icon_dir_closed.clear();
            self.icon_fallback.clear();
        }

        // Provider registrations (language / fold / etc.).
        self.plugin_manager.remove_provider_registrations(name);

        // Re-render the current file without plugin content.
        if had_current_content {
            self.plugin_content_active = false;
            self.plugin_content_active_path = None;
            self.reload_content();
        }
    }

    /// Rebuilds the file tree, re-fetches git status (async), and reloads the
    /// current file. Triggered explicitly by the reload key, by debounced
    /// filesystem events from the root watcher, or by the periodic fallback
    /// timer when no root watcher is installed.
    pub fn reload(&mut self) {
        self.last_refresh = self.now();
        if self.git_status_enabled {
            self.request_git_status_refresh();
        }
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        let changed = self.git_changed_files_set();
        if let Some(s) = &mut self.search {
            s.reload_files(&root, show_hidden, ignore_gitignore, changed.as_ref());
        }
        self.rebuild(false);
        self.reload_content();
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

    /// Checks if a command is currently applicable based on the application state.
    /// Returns `Ok(())` if applicable, or `Err(reason)` if inapplicable.
    pub fn check_applicability(&self, action_id: &str) -> Result<(), &'static str> {
        let action = crate::actions::ACTIONS.iter().find(|a| a.id == action_id);
        let applicability = match action {
            Some(a) => a.applicability(),
            None => return Ok(()),
        };
        match applicability {
            crate::actions::Applicability::Always => Ok(()),
            crate::actions::Applicability::OpenFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                Ok(())
            }
            crate::actions::Applicability::JsonFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if !self.is_json {
                    return Err("requires JSON file");
                }
                if self.json_pretty_lines.is_empty() {
                    return Err("JSON file failed to parse");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepo => {
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndNoDiff => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                if self.is_diff {
                    return Err("not available in a diff");
                }
                if !self.has_text_cursor() {
                    return Err("not available (current file not plugin-rendered)");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndDiffView => {
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                if !self.is_diff {
                    return Err("requires diff view");
                }
                Ok(())
            }
            crate::actions::Applicability::DiffView => {
                if !self.is_diff {
                    return Err("requires diff view");
                }
                Ok(())
            }
            crate::actions::Applicability::FoldRegions => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.fold_regions.is_empty() {
                    return Err("no fold regions in file");
                }
                Ok(())
            }
            crate::actions::Applicability::PluginContentActive => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if !self.plugin_content_active {
                    return Err("not available (current file not plugin-rendered)");
                }
                Ok(())
            }
            crate::actions::Applicability::GitMode => {
                if !self.git_mode {
                    return Err("requires git mode");
                }
                Ok(())
            }
        }
    }

    /// Flips `[telemetry] enabled`, rebuilds the live `Telemetry` handle to
    /// match (spawning or tearing down its writer thread), persists the
    /// change, and reports the new state in the status bar.
    pub(crate) fn toggle_telemetry(&mut self) {
        let enabled = !self.config.telemetry.enabled;
        self.config.telemetry.enabled = enabled;
        self.telemetry = crate::telemetry::Telemetry::new(enabled);
        if enabled && !self.config.telemetry.notice_shown {
            self.config.telemetry.notice_shown = true;
            self.show_telemetry_notice = true;
        }
        self.save_config();
        self.set_status(format!(
            "telemetry {}",
            if enabled { "enabled" } else { "disabled" }
        ));
    }

    /// Collects an anonymous diagnostic report, saves it under the state directory,
    /// and opens the user's browser pre-filled with a GitHub new-issue URL.
    /// If the report exceeds ~6KB, truncates the URL query and copies the full
    /// report to the clipboard instead. If browser launch fails, falls back
    /// to clipboard copying and status bar instructions.
    pub(crate) fn save_bug_report(&mut self) {
        let report = crate::diagnostics::DiagnosticReport::collect(self);
        let md = report.to_markdown();
        let body_text = report.body.clone();

        // 1. Save locally first.
        let local_path = match report.save() {
            Ok(path) => Some(path),
            Err(e) => {
                self.telemetry
                    .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                        module: "diagnostics",
                        kind: "bug_report_failed",
                    });
                self.set_status(format!("bug report local save failed: {e}"));
                None
            }
        };

        // 2. Prepare URL parameters.
        let base = "https://github.com/ansromanov/mantis/issues/new?template=app-bugreport.yml";
        let title_encoded = percent_encode("App Bug Report");

        let mut truncated = false;
        let (description_encoded, diagnostics_encoded) = {
            let desc_enc = percent_encode(&body_text);
            let diag_enc = percent_encode(&md);
            let test_url = format!(
                "{}&title={}&description={}&diagnostics={}",
                base, title_encoded, desc_enc, diag_enc
            );
            if test_url.len() > 6000 {
                truncated = true;
                let fallback_msg = format!(
                    "<!-- DIAGNOSTICS TOO LARGE FOR URL - COPIED TO CLIPBOARD. PLEASE PASTE OVER THIS LINE -->\n\
                     [Report truncated due to URL length limit ({} bytes). The full report was copied to your clipboard. Please paste it here.]",
                    md.len()
                );
                (percent_encode(&fallback_msg), percent_encode(""))
            } else {
                (desc_enc, diag_enc)
            }
        };

        let url = format!(
            "{}&title={}&description={}&diagnostics={}",
            base, title_encoded, description_encoded, diagnostics_encoded
        );

        // 3. Try to open browser (unless in a test context).
        let browser_res = if cfg!(test) {
            Ok(())
        } else {
            self.open_in_browser(&url)
        };

        // 4. Update status and clipboard depending on truncation and browser success.
        match (browser_res, truncated) {
            (Ok(()), false) => {
                if let Some(path) = local_path {
                    self.set_status(format!("bug report saved: {}", path.display()));
                } else {
                    self.set_status("opened browser");
                }
            }
            (Ok(()), true) => {
                self.copy_to_clipboard(md, "full bug report");
                self.set_status("opened browser; full report copied (paste into description)");
            }
            (Err(err), _) => {
                self.telemetry
                    .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                        module: "diagnostics",
                        kind: "bug_report_failed",
                    });
                self.copy_to_clipboard(md, "full bug report");
                self.set_status(format!(
                    "clipboard filled; browser failed (open github.com/ansromanov/mantis/issues/new): {err}"
                ));
            }
        }

        self.bug_report = None;
    }

    /// Copies `text` to the system clipboard, reporting success or failure in the
    /// status bar. Single source of truth for clipboard writes.
    pub(crate) fn copy_to_clipboard(&mut self, text: String, label: &str) {
        if text.is_empty() {
            return;
        }
        match self.clipboard_set(text) {
            Ok(()) => self.set_status(format!("copied {label}")),
            Err(e) => {
                self.telemetry
                    .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                        module: "clipboard",
                        kind: "write_failed",
                    });
                self.set_status(format!("clipboard error: {e}"));
            }
        }
    }

    /// Writes `text` to the system clipboard.
    #[cfg(not(test))]
    fn clipboard_set(&mut self, text: String) -> Result<(), arboard::Error> {
        arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text))
    }

    /// Test double: captures `text` in `clipboard_capture` so `cargo test`
    /// never touches (or races on) the real system clipboard.
    #[cfg(test)]
    fn clipboard_set(&mut self, text: String) -> Result<(), arboard::Error> {
        self.clipboard_capture.push(text);
        Ok(())
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

    /// Human-readable label for the tree panel's current view mode, shown as the
    /// panel's border title. Extends as new modes are added (blame, functions,
    /// file structure, …); today it reflects only Files vs Git vs Blame.
    pub fn panel_mode_label(&self) -> &'static str {
        if self.show_blame && self.has_text_cursor() {
            "Blame"
        } else if self.git_mode {
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
    /// JSON, scroll, selection, fold state, plugin content, etc.).
    /// Called when the current file is no longer valid — git mode becomes clean
    /// (no changed files) or the viewer root changes. Does NOT clear plugin
    /// contributions or file watchers (those are managed at the call site).
    fn clear_content_state(&mut self) {
        self.current_file = None;
        self.current_syntax = None;
        self.content = Vec::new();
        self.highlighted = Vec::new();
        self.virtual_file = None;
        self.is_json = false;
        self.file_encoding = None;
        self.file_line_ending = None;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.viewing_revision = None;
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
        self.plugin_content_active = false;
        self.plugin_content.clear();
        self.plugin_content_text.clear();
    }
}

/// Toggles xterm alternate-scroll mode (DECSET 1007), which otherwise
/// translates mouse-wheel events into arrow-key presses in the alternate
/// screen. At a scroll bound (e.g. wheel-up at the first line) those
/// synthetic key presses cause visible flashing/tearing even though mantis's
/// own scroll state is a no-op. Best-effort: write errors are ignored.
///
/// mantis always restores this to *enabled* on exit rather than probing and
/// restoring whatever the terminal's mode was before mantis started (that
/// requires a synchronous DECRQM query/response round-trip). If the ambient
/// terminal had it disabled, exiting mantis will leave it enabled.
pub(crate) fn set_alternate_scroll(enabled: bool) {
    use std::io::Write;
    let sequence = if enabled {
        "\x1b[?1007h"
    } else {
        "\x1b[?1007l"
    };
    let _ = write!(std::io::stdout(), "{sequence}");
    let _ = std::io::stdout().flush();
}

/// Restores the terminal to a normal state: pops keyboard enhancement flags
/// (Unix only), disables raw mode, leaves the alternate screen, disables mouse
/// capture, restores alternate-scroll mode, and shows the cursor. Best-effort
/// and idempotent — each operation is silently ignored on error so this can be
/// called from a panic hook or from regular teardown.
#[cfg_attr(not(unix), allow(unused_imports))]
pub(crate) fn restore_terminal() {
    use std::io;

    use crossterm::cursor::Show;
    use crossterm::event::DisableMouseCapture;
    use crossterm::execute;
    use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};

    let _ = crate::event_source::pop_keyboard_enhancement_flags();
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    );
    set_alternate_scroll(true);
}

fn percent_encode(s: &str) -> String {
    let mut encoded = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", b));
            }
        }
    }
    encoded
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
