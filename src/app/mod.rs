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

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::fold::FoldRegion;
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::plugin::{self, ExtraSyntax, PluginContributions, PluginManager};
use crate::search::{
    CommandPalette, GotoLineState, HistoryState, InFileSearch, PluginPicker, RecentFilesState,
    SearchState, ThemePicker, TreeFilter,
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
mod refresh;

use loader::Loader;

/// Which panel is currently focused.
#[derive(Debug, PartialEq)]
pub enum Focus {
    /// The file tree panel on the left.
    Tree,
    /// The file content / diff panel on the right.
    Content,
}

/// Which git diff view is active in the content pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiffMode {
    /// All changes vs HEAD (`git diff HEAD`) — the default.
    #[default]
    All,
    /// Only staged changes (`git diff --cached`).
    Staged,
    /// Only unstaged changes (`git diff`).
    Unstaged,
}

impl DiffMode {
    /// Cycles through All -> Staged -> Unstaged -> All.
    pub fn next(self) -> Self {
        match self {
            DiffMode::All => DiffMode::Staged,
            DiffMode::Staged => DiffMode::Unstaged,
            DiffMode::Unstaged => DiffMode::All,
        }
    }

    /// Short label used in the content title badge.
    pub fn label(self) -> &'static str {
        match self {
            DiffMode::All => "all",
            DiffMode::Staged => "staged",
            DiffMode::Unstaged => "unstaged",
        }
    }
}

/// Git info provided by a plugin for the status bar, replacing the live
/// `git::repo_info()` call when set.
pub struct PluginGitInfo {
    /// Branch name (e.g. "main", "feature/x").
    pub branch: String,
    /// Short commit hash (e.g. "abc1234").
    #[allow(dead_code)]
    pub head: String,
    /// Whether the working tree is dirty.
    pub dirty: bool,
    /// State label: "clean", "dirty", "conflict", "rebase", or "merge".
    pub state: String,
}

/// A transient status message with a timestamp so it can auto-expire.
#[derive(Debug)]
pub struct StatusMessage {
    pub text: String,
    pub set_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>, now: Instant) -> Self {
        Self {
            text: text.into(),
            set_at: now,
        }
    }

    /// Returns `true` when the message has been alive for at least `ttl`.
    pub fn expired(&self, ttl: Duration) -> bool {
        self.set_at.elapsed() >= ttl
    }
}

/// Central application state. Holds the file tree, content buffers, overlay
/// state, geometry captured during rendering, and configuration.
pub struct App {
    pub root: PathBuf,
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
    pub markdown_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    pub virtual_file: Option<VirtualFile>,
    pub is_markdown: bool,
    pub show_raw_markdown: bool,
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
    /// Side-by-side rows parsed from the current diff; empty for non-diffs.
    pub diff_rows: Vec<crate::diff::DiffRow>,
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
    pub tree_width: u16,
    pub show_help: bool,
    pub should_quit: bool,
    pub theme: Theme,
    pub git_status_enabled: bool,
    pub git_show_deleted: bool,
    pub git_info: Option<crate::git::GitRepoInfo>,
    pub git_status_map: HashMap<PathBuf, GitStatus>,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    pub show_scrollbar: bool,
    pub show_scroll_percentage: bool,
    pub show_line_numbers: bool,
    pub show_blame: bool,
    pub show_about: bool,
    pub walk_errors: usize,
    /// Warning describing a malformed config that was ignored at startup, if any.
    pub config_error: Option<String>,
    /// Whether to automatically reload file content on disk change.
    pub auto_watch: bool,
    pub show_file_info: bool,
    pub indent_guides: bool,
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
    config: Config,
    config_path: Option<std::path::PathBuf>,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
    /// The set of node indices (into `nodes`) that were visible during the last
    /// tree render. Populated by `draw_tree` when `tree_filter` is active so
    /// mouse handlers can map screen rows back to global indices.
    pub tree_visible_indices: Vec<usize>,
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
    /// Plugin subprocess manager. Spawned at startup, deactivated on quit.
    pub(crate) plugin_manager: PluginManager,
    /// Set to `true` while handling a plugin-originated `open_file` action, so
    /// `apply_file_load` / `apply_diff_load` can suppress the re-emission of
    /// `on_file_open` back to plugins (breaking the recursion loop).
    plugin_is_opening_file: bool,
    /// Most recent plugin message, shown in the status bar.
    pub plugin_message: Option<String>,
    /// Tracks what application state each plugin has contributed so that
    /// disabling or crashing the plugin tears down exactly its output.
    /// Populated by `handle_plugin_action` and consumed by
    /// `teardown_plugin_contributions`.
    pub(crate) plugin_contributions: HashMap<String, PluginContributions>,
    /// Per-file blame annotations provided by a plugin, keyed by absolute path.
    /// Each entry is a Vec of formatted blame strings (one per line, 0-indexed).
    /// Checked before the live `git::file_blame()` call in the content pane.
    pub plugin_blame: HashMap<PathBuf, Vec<String>>,
    /// Git branch/HEAD/dirty/state info provided by a plugin for the status bar.
    /// When set, displayed instead of the live `git_info`.
    pub plugin_git_info: Option<PluginGitInfo>,
    /// Plugin-rendered content keyed by file path. Populated by the `set_content`
    /// action; the content pane checks this before markdown/virtual-file rendering.
    pub plugin_content: HashMap<PathBuf, Vec<Vec<(ratatui::style::Style, String)>>>,
    /// Plain text of plugin-provided content, keyed by file path (same keys as
    /// `plugin_content`). Used for in-file search, selection, and blame display.
    /// Stored separately because `plugin_content` holds styled spans whose text
    /// would require joining on every access.
    pub plugin_content_text: HashMap<PathBuf, Vec<String>>,
    /// Set to `true` when a plugin sends `set_content` for the current file and
    /// reset to `false` when `current_file` changes, so the `[rendering…]`
    /// placeholder only shows while the plugin is actively working on that file.
    pub plugin_content_active: bool,
    /// Transient status message (e.g. "path copied"), shown until the next keypress
    /// or auto-expires after ~3 seconds.
    pub status_message: Option<StatusMessage>,
    /// Breadcrumb segment areas recorded during the last render, used for mouse
    /// hit-testing. Each entry is (target_directory_path, clickable_rect).
    pub breadcrumb_areas: Vec<(std::path::PathBuf, Rect)>,
    /// When `true`, the session cache needs to be re-written.
    session_dirty: bool,
    /// When the session was last dirtied, for debounced writes.
    session_dirty_at: Option<std::time::Instant>,
    /// Last time the session cache was flushed to disk.
    session_last_save: std::time::Instant,
}

impl App {
    /// Persists the current config to disk if a config path was provided.
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            config::save(&self.config, path);
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
        };
        crate::session::save(&self.root, &state);
        self.session_dirty = false;
        self.session_dirty_at = None;
        self.session_last_save = self.now();
    }

    /// Tears down all application state produced by the named plugin.
    ///
    /// Removes content, blame, file-status, fold-region, git-info, and icon-map
    /// contributions, clears the plugin's provider registrations, and reloads
    /// the current file if the plugin had rendered content for it — so the
    /// display falls back to core rendering (markdown, JSON, or plain text).
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

        // Blame data.
        for path in &contrib.blame_paths {
            self.plugin_blame.remove(path);
        }

        // File statuses — remove only the paths this plugin contributed.
        for path in &contrib.status_paths {
            self.git_status_map.remove(path);
        }

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

        // Git info (status bar override).
        if contrib.has_git_info {
            self.plugin_git_info = None;
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
            self.reload_content();
        }
    }

    /// Toggles the currently highlighted plugin in the picker: spawns it if
    /// stopped, kills it if running, or flips the enabled flag for syntax
    /// plugins and reloads syntax definitions so the change takes effect
    /// immediately. Updates `config.plugins[name].enabled` and writes `tv.toml`
    /// so the change persists across restarts.
    pub(crate) fn toggle_plugin_picker_selection(&mut self) {
        let Some(picker) = &self.plugin_picker else {
            return;
        };
        let Some((name, running, kind)) = picker.entries.get(picker.selected).cloned() else {
            return;
        };
        if kind == plugin::PluginKind::Syntax {
            // Syntax plugin: just flip the enabled flag and rebuild syntaxes.
            let was_enabled = self
                .config
                .plugins
                .get(&name)
                .map(|e| e.enabled)
                .unwrap_or(false);
            if let Some(entry) = self.config.plugins.get_mut(&name) {
                entry.enabled = !was_enabled;
            }
            self.plugin_manager.set_enabled(&name, !was_enabled);
            self.save_config();
            self.rebuild_extra_syntaxes();
            self.reload_content();
        } else if running {
            self.plugin_manager.deactivate_one(&name);
            if let Some(entry) = self.config.plugins.get_mut(&name) {
                entry.enabled = false;
            }
            self.save_config();
            // Tear down all state this plugin produced, then re-render the
            // current file without plugin content. This replaces the former
            // per-plugin-name special case (e.g. `if name == "iconize"`).
            self.teardown_plugin_contributions(&name);
        } else {
            // Ensure the plugin file is present on disk before spawning.
            plugin::install_bundled_plugins();
            match self
                .plugin_manager
                .activate_one(&name, self.current_file.as_deref())
            {
                Ok(()) => {
                    if let Some(entry) = self.config.plugins.get_mut(&name) {
                        entry.enabled = true;
                    }
                    self.save_config();
                }
                Err(e) => {
                    self.plugin_message = Some(format!("Plugin error: {e}"));
                }
            }
        }
        let updated = self.plugin_manager.plugin_entries();
        if let Some(picker) = &mut self.plugin_picker {
            picker.entries = updated;
        }
    }

    /// Rebuilds the `extra_syntaxes` list from the current config and updates
    /// the main-thread highlighter and the worker thread's highlighter so that
    /// syntax highlighting reflects the latest set of enabled syntax plugins.
    fn rebuild_extra_syntaxes(&mut self) {
        let mut plugin_entries: Vec<_> = self.config.plugins.clone().into_iter().collect();
        plugin_entries.sort_by(|a, b| a.0.cmp(&b.0));
        self.extra_syntaxes = plugin::load_extra_syntaxes(&plugin_entries);
        self.highlighter = crate::highlight::Highlighter::with_extra_syntaxes(
            &self.theme.syntax,
            &self.extra_syntaxes,
        );
        self.loader_set_extra_syntaxes();
    }

    /// Rebuilds the file tree, re-fetches git status, and reloads the current
    /// file. Triggered explicitly by the reload key, by debounced filesystem
    /// events from the root watcher, or by the periodic fallback timer when no
    /// root watcher is installed.
    pub fn reload(&mut self) {
        self.last_refresh = self.now();
        if self.git_status_enabled {
            #[cfg(feature = "git-core")]
            {
                self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
                self.git_info = crate::git::repo_info(&self.root);
            }
        }
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        if let Some(s) = &mut self.search {
            s.reload_files(&root, show_hidden, ignore_gitignore);
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

    /// Records that the user scrolled the content, used to show a transient
    /// scrollbar.
    pub fn mark_content_scrolled(&mut self) {
        self.content_scrolled_at = self.now();
    }

    pub fn keys(&self) -> &Keymap {
        &self.keys
    }
}

/// Builds the set of absolute paths that should appear as ghost (deleted) nodes
/// in the tree. Only files that are absent from the working tree are included.
fn deleted_set(map: &HashMap<PathBuf, GitStatus>, enabled: bool) -> HashSet<PathBuf> {
    if !enabled {
        return HashSet::new();
    }
    map.iter()
        .filter(|(path, &status)| status == GitStatus::Deleted && !path.exists())
        .map(|(path, _)| path.clone())
        .collect()
}

/// Returns `true` when `(col, row)` lies within the given `Rect`.
fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && col < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

/// Colors a unified-diff line by its leading marker.
fn diff_line_style(line: &str, theme: &Theme) -> ratatui::style::Style {
    use ratatui::style::{Modifier, Style};
    if line.starts_with("@@") {
        Style::default().fg(theme.accent)
    } else if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(theme.dim).add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(theme.diff_add)
    } else if line.starts_with('-') {
        Style::default().fg(theme.diff_del)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(theme.dim)
    } else {
        Style::default()
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
