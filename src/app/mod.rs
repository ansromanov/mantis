//! Central application state: the `App` struct that ties the whole TUI together.
//!
//! `App` holds the file tree, content/diff buffers, every overlay's state
//! (search, history, theme picker, command palette, recent files, help, about, blame), the
//! resolved theme and keymap, and the geometry captured during the last render
//! so mouse handlers can hit-test clicks. Construction (`App::new`) walks the
//! root, loads git status, and opens the first file; `reload`/`tick` keep the
//! view in sync with the filesystem via a debounced watcher with a periodic
//! fallback. Behaviour is split across sibling submodules (key/mouse handlers,
//! navigation, file_ops, loader, refresh, content/diff/yaml helpers); this file
//! owns the struct, its fields, and a few shared free functions.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::plugin::PluginManager;
use crate::search::{
    CommandPalette, HistoryState, InFileSearch, RecentFilesState, SearchState, ThemePicker,
};
use crate::selection::{TextSelection, VisualLine};
use crate::theme::Theme;
use crate::tree::{build_visible, TreeNode};
use crate::virtual_file::VirtualFile;
use crate::yaml_fold::FoldRegion;

mod content_pos;
mod content_query;
mod diff_nav;
mod file_ops;
mod key_handlers;
mod loader;
mod mouse_handlers;
mod navigation;
mod refresh;
mod yaml_fold;

use loader::Loader;

/// Which panel is currently focused.
#[derive(Debug, PartialEq)]
pub enum Focus {
    /// The file tree panel on the left.
    Tree,
    /// The file content / diff panel on the right.
    Content,
}

/// Central application state. Holds the file tree, content buffers, overlay
/// state, geometry captured during rendering, and configuration.
pub struct App {
    pub root: PathBuf,
    pub nodes: Vec<TreeNode>,
    pub expanded: HashSet<PathBuf>,
    pub tree_selected: usize,
    /// Viewport top offset for the tree panel. Only used when
    /// `tree_independent_scroll` is enabled; otherwise the tree auto-scrolls to
    /// keep `tree_selected` visible and this stays in sync with that.
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
    pub word_wrap: bool,
    pub current_file: Option<PathBuf>,
    pub is_diff: bool,
    /// When `true`, diffs render in a split old|new layout instead of unified.
    pub diff_side_by_side: bool,
    /// Side-by-side rows parsed from the current diff; empty for non-diffs.
    pub diff_rows: Vec<crate::diff::DiffRow>,
    pub content_title: Option<String>,
    pub focus: Focus,
    pub search: Option<SearchState>,
    pub last_search_query: String,
    pub in_file_search: Option<InFileSearch>,
    pub command_palette: Option<CommandPalette>,
    pub history: Option<HistoryState>,
    pub theme_picker: Option<ThemePicker>,
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
    keys: Keymap,
    config: Config,
    config_path: Option<std::path::PathBuf>,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
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
    /// Active visual-line selection in the content panel, if any. Whole lines
    /// are selected and a scoped git-blame panel can be opened for the range.
    pub visual_line: Option<VisualLine>,
    /// Whether the selection-scoped git-blame panel is open. Only meaningful
    /// while `visual_line` is `Some`.
    pub blame_panel: bool,
    drag_start: Option<(usize, usize)>,
    scrollbar_drag: bool,
    splitter_drag: bool,
    /// Set to `true` after suspending the TUI (e.g. for editor), signals
    /// `main.rs` to call `terminal.clear()` before the next `draw()`.
    pub needs_clear: bool,
    // YAML folding state
    pub yaml_fold_regions: Vec<FoldRegion>,
    pub yaml_folded: HashSet<usize>,
    /// display_line → physical_line mapping; empty when no folds are active.
    pub fold_display_map: Vec<usize>,
    /// (screen_y, region_idx) pairs recorded during the last render, used for
    /// fold-gutter mouse click detection.
    pub fold_gutter_rows: Vec<(u16, usize)>,
    /// YAML parse error message, if any (set when opening a `.yaml`/`.yml` file).
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
    pub plugin_manager: PluginManager,
    /// Most recent plugin message, shown in the status bar.
    pub plugin_message: Option<String>,
    /// Transient status message (e.g. "path copied"), shown until the next keypress.
    pub status_message: Option<String>,
    /// Breadcrumb segment areas recorded during the last render, used for mouse
    /// hit-testing. Each entry is (target_directory_path, clickable_rect).
    pub breadcrumb_areas: Vec<(std::path::PathBuf, Rect)>,
}

impl App {
    /// Builds the app: walks the root directory, loads git status, resolves
    /// the theme, and opens the first selected file.
    pub fn new(
        root: PathBuf,
        cfg: Config,
        config_path: Option<std::path::PathBuf>,
        config_error: Option<String>,
    ) -> anyhow::Result<Self> {
        let expanded = HashSet::new();
        // git_mode requires status data even if git_status is disabled in config.
        let git_status_enabled = cfg.git_status || cfg.git_mode;
        let git_show_deleted = cfg.git_show_deleted;
        let git_status_map = if git_status_enabled {
            crate::git::repo_status(&root, cfg.ignore_gitignore)
        } else {
            HashMap::new()
        };
        let git_info = if git_status_enabled {
            crate::git::repo_info(&root)
        } else {
            None
        };
        let deleted = deleted_set(&git_status_map, git_show_deleted);
        let (nodes, walk_errors) = build_visible(
            &root,
            &expanded,
            cfg.show_hidden,
            cfg.ignore_gitignore,
            &deleted,
        );
        let theme = cfg.theme.resolve();
        let saved_config = cfg.clone();
        let highlighter = Highlighter::new(&theme.syntax);
        let loader = Loader::new(&theme);
        let mut plugin_entries: Vec<_> = cfg.plugins.clone().into_iter().collect();
        plugin_entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut plugin_manager = PluginManager::new(plugin_entries);
        plugin_manager.activate_all();
        let plugin_spawn_error = plugin_manager
            .take_spawn_errors()
            .into_iter()
            .next()
            .map(|e| format!("[plugin] {e}"));
        let mut app = App {
            root,
            nodes,
            expanded,
            tree_selected: 0,
            tree_scroll: 0,
            tree_independent_scroll: cfg.tree_independent_scroll,
            content: Vec::new(),
            highlighted: Vec::new(),
            markdown_lines: Vec::new(),
            virtual_file: None,
            is_markdown: false,
            show_raw_markdown: false,
            is_json: false,
            file_encoding: None,
            file_line_ending: None,
            show_pretty_json: false,
            json_pretty_text: Vec::new(),
            json_pretty_lines: Vec::new(),
            content_scroll: 0,
            content_hscroll: 0,
            word_wrap: cfg.word_wrap,
            current_file: None,
            is_diff: false,
            diff_side_by_side: false,
            diff_rows: Vec::new(),
            content_title: None,
            focus: Focus::Tree,
            search: None,
            last_search_query: String::new(),
            in_file_search: None,
            command_palette: None,
            history: None,
            theme_picker: None,
            recent_ring: Vec::new(),
            recent_files: None,
            recent_area: Rect::default(),
            recent_offset: 0,
            show_hidden: cfg.show_hidden,
            ignore_gitignore: cfg.ignore_gitignore,
            tree_width: cfg.tree_width,
            show_help: false,
            should_quit: false,
            theme,
            git_status_enabled,
            git_show_deleted,
            git_info,
            git_status_map,
            git_mode: cfg.git_mode,
            git_mode_flat: cfg.git_mode_flat,
            show_scrollbar: cfg.scrollbar,
            show_scroll_percentage: cfg.scroll_percentage,
            show_line_numbers: cfg.line_numbers,
            show_blame: false,
            show_about: false,
            walk_errors,
            config_error,
            auto_watch: cfg.watch,
            show_file_info: cfg.show_file_info,
            keys: cfg.keys,
            config: saved_config,
            config_path,
            tree_area: Rect::default(),
            tree_offset: 0,
            content_area: Rect::default(),
            search_area: Rect::default(),
            search_offset: 0,
            command_palette_area: Rect::default(),
            command_palette_offset: 0,
            history_area: Rect::default(),
            history_offset: 0,
            theme_area: Rect::default(),
            theme_offset: 0,
            splitter_area: Rect::default(),
            last_click: None,
            content_scrolled_at: Instant::now() - std::time::Duration::from_secs(10),
            highlighter,
            last_refresh: Instant::now(),
            file_watcher: None,
            file_watch_rx: None,
            file_watch_path: None,
            root_watcher: None,
            root_watch_rx: None,
            tree_dirty: false,
            tree_dirty_at: None,
            selection: None,
            visual_line: None,
            blame_panel: false,
            drag_start: None,
            scrollbar_drag: false,
            splitter_drag: false,
            needs_clear: false,
            yaml_fold_regions: Vec::new(),
            yaml_folded: HashSet::new(),
            fold_display_map: Vec::new(),
            fold_gutter_rows: Vec::new(),
            yaml_error: None,
            yaml_anchor_count: 0,
            yaml_alias_count: 0,
            loader,
            load_seq: 0,
            loading: false,
            plugin_manager,
            plugin_message: plugin_spawn_error,
            status_message: None,
            breadcrumb_areas: Vec::new(),
        };
        if app.git_mode {
            app.expand_git_dirs();
            app.rebuild();
        }
        // Open the first file synchronously so it is visible on the first frame
        // (and so callers/tests can observe content right after construction).
        app.open_selected_sync();
        Ok(app)
    }

    /// Persists the current config to disk if a config path was provided.
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            config::save(&self.config, path);
        }
    }

    /// Rebuilds the file tree, re-fetches git status, and reloads the current
    /// file. Triggered explicitly by the reload key, by debounced filesystem
    /// events from the root watcher, or by the periodic fallback timer when no
    /// root watcher is installed.
    pub fn reload(&mut self) {
        self.last_refresh = Instant::now();
        if self.git_status_enabled {
            self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
            self.git_info = crate::git::repo_info(&self.root);
        }
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        if let Some(s) = &mut self.search {
            s.reload_files(&root, show_hidden, ignore_gitignore);
        }
        self.rebuild();
        self.reload_content();
    }

    /// Records that the user scrolled the content, used to show a transient
    /// scrollbar.
    pub fn mark_content_scrolled(&mut self) {
        self.content_scrolled_at = Instant::now();
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
