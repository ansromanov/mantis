use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::search::{CommandPalette, HistoryState, InFileSearch, SearchState, ThemePicker};
use crate::selection::{TextSelection, VisualLine};
use crate::theme::Theme;
use crate::tree::{build_visible, TreeNode};
use crate::virtual_file::VirtualFile;
use crate::yaml_fold::FoldRegion;

mod content_pos;
mod file_ops;
mod key_handlers;
mod loader;
mod mouse_handlers;
mod navigation;

use loader::{LoadRequest, LoadResponse, Loader};

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
    pub show_blame: bool,
    pub show_about: bool,
    pub walk_errors: usize,
    /// Warning describing a malformed config that was ignored at startup, if any.
    pub config_error: Option<String>,
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
            show_blame: false,
            show_about: false,
            walk_errors,
            config_error,
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

    /// Debounce window: how long the tree must stay quiet after a filesystem
    /// event before a reload runs. Coalesces bursts (e.g. a build touching many
    /// files) into a single refresh.
    ///
    /// In test builds the window is inflated to 60 s so that the debounce tests
    /// can assert "still fresh" without relying on sub-300 ms scheduling.
    #[cfg(not(test))]
    const TREE_RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
    #[cfg(test)]
    const TREE_RELOAD_DEBOUNCE: Duration = Duration::from_secs(60);

    /// Per-frame update. Refreshes the open file from its watcher, advances the
    /// debounced content search, and drives the tree/git refresh: when the root
    /// watcher is installed this is event-driven (reload only after the tree has
    /// been quiet for `TREE_RELOAD_DEBOUNCE`); otherwise it falls back to a
    /// periodic reload so the view never goes permanently stale.
    pub fn tick(&mut self) {
        self.drain_loads();
        if self.drain_file_watch() {
            self.reload_content();
        }
        if let Some(ref mut s) = self.search {
            s.maybe_refresh();
        }
        if self.drain_root_watch() {
            self.tree_dirty = true;
            self.tree_dirty_at = Some(Instant::now());
        }
        if self.tree_dirty {
            // Wait for the tree to go quiet before reloading so a burst of events
            // produces one refresh, not one per event.
            let quiet = self
                .tree_dirty_at
                .is_some_and(|t| t.elapsed() >= Self::TREE_RELOAD_DEBOUNCE);
            if quiet {
                self.tree_dirty = false;
                self.tree_dirty_at = None;
                self.reload();
            }
        } else if self.root_watcher.is_none() && self.last_refresh.elapsed().as_secs() >= 30 {
            // No watcher (install failed): fall back to a blind periodic reload.
            self.reload();
        }
    }

    /// Width of the fold gutter (2 chars: marker + space) when YAML regions
    /// are detected, 0 otherwise.
    pub fn fold_gutter_width(&self) -> usize {
        if self.yaml_fold_regions.is_empty() {
            0
        } else {
            2
        }
    }

    /// Whether the diff should currently render in the side-by-side layout: the
    /// toggle is on, a diff is loaded, and the content pane is wide enough.
    pub fn diff_sbs_active(&self) -> bool {
        self.is_diff
            && self.diff_side_by_side
            && !self.diff_rows.is_empty()
            && self.content_area.width >= crate::diff::MIN_SIDE_BY_SIDE_WIDTH
    }

    /// Returns the number of **display** lines after folding. Equals
    /// `line_count()` when no folds are active.
    pub fn display_line_count(&self) -> usize {
        if self.diff_sbs_active() {
            self.diff_rows.len()
        } else if self.fold_display_map.is_empty() {
            self.line_count()
        } else {
            self.fold_display_map.len()
        }
    }

    /// Maps a display-space line index to a physical file line index.
    pub fn display_to_physical(&self, display: usize) -> usize {
        if self.fold_display_map.is_empty() {
            display
        } else {
            self.fold_display_map
                .get(display)
                .copied()
                .unwrap_or(display)
        }
    }

    /// Converts a physical line index to a display line index.
    /// When folding is inactive this is identity; when active it finds the
    /// position of `physical` in the display map (first visible line ≥ physical
    /// when the line itself is hidden inside a fold).
    pub fn physical_to_display(&self, physical: usize) -> usize {
        if self.fold_display_map.is_empty() {
            return physical;
        }
        // Find the first display line whose physical index is >= physical.
        self.fold_display_map
            .iter()
            .position(|&p| p >= physical)
            .unwrap_or(self.fold_display_map.len().saturating_sub(1))
    }

    /// Returns the display-row indices of hunk headers (`@@`) in the current
    /// diff, in the coordinate space matching the active layout.
    fn diff_hunk_rows(&self) -> Vec<usize> {
        if self.diff_sbs_active() {
            self.diff_rows
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r, crate::diff::DiffRow::Header(_)))
                .map(|(i, _)| i)
                .collect()
        } else {
            self.content
                .iter()
                .enumerate()
                .filter(|(_, l)| l.starts_with("@@"))
                .map(|(i, _)| i)
                .collect()
        }
    }

    /// Scrolls to the next hunk header below the current scroll position.
    pub(crate) fn diff_next_hunk(&mut self) {
        let cur = self.content_scroll;
        if let Some(&next) = self.diff_hunk_rows().iter().find(|&&i| i > cur) {
            self.content_scroll = next.min(self.content_scroll_max());
            self.mark_content_scrolled();
        }
    }

    /// Scrolls to the previous hunk header above the current scroll position.
    pub(crate) fn diff_prev_hunk(&mut self) {
        let cur = self.content_scroll;
        if let Some(&prev) = self.diff_hunk_rows().iter().rev().find(|&&i| i < cur) {
            self.content_scroll = prev;
            self.mark_content_scrolled();
        }
    }

    /// Returns the fold region index whose `start` matches `physical_line`, if any.
    pub fn region_idx_at(&self, physical_line: usize) -> Option<usize> {
        self.yaml_fold_regions
            .iter()
            .position(|r| r.start == physical_line)
    }

    /// Rebuilds `fold_display_map` from the current `yaml_folded` set.
    pub fn rebuild_fold_display_map(&mut self) {
        self.fold_display_map = crate::yaml_fold::build_display_map(
            &self.yaml_fold_regions,
            &self.yaml_folded,
            self.line_count(),
        );
    }

    /// Toggles the fold state of `region_idx` and clamps the scroll position.
    pub fn toggle_fold_region(&mut self, region_idx: usize) {
        if self.yaml_folded.contains(&region_idx) {
            self.yaml_folded.remove(&region_idx);
        } else {
            self.yaml_folded.insert(region_idx);
        }
        self.rebuild_fold_display_map();
        self.content_scroll = self.content_scroll.min(self.content_scroll_max());
    }

    /// Folds every detected YAML region and scrolls to the top.
    pub fn fold_all(&mut self) {
        self.yaml_folded = (0..self.yaml_fold_regions.len()).collect();
        self.rebuild_fold_display_map();
        self.content_scroll = 0;
    }

    /// Expands every YAML region.
    pub fn unfold_all(&mut self) {
        self.yaml_folded.clear();
        self.fold_display_map.clear();
    }

    /// Resets all YAML state. Called whenever a new file is opened.
    pub(crate) fn clear_yaml_state(&mut self) {
        self.yaml_fold_regions.clear();
        self.yaml_folded.clear();
        self.fold_display_map.clear();
        self.fold_gutter_rows.clear();
        self.yaml_error = None;
        self.yaml_anchor_count = 0;
        self.yaml_alias_count = 0;
    }

    /// Returns the total number of lines in the current content source
    /// (virtual file, raw content, or markdown-rendered lines).
    pub fn line_count(&self) -> usize {
        if self.is_markdown && !self.show_raw_markdown {
            self.markdown_lines.len()
        } else if self.is_json && self.show_pretty_json && !self.json_pretty_lines.is_empty() {
            self.json_pretty_lines.len()
        } else if let Some(vf) = &self.virtual_file {
            vf.line_count()
        } else {
            self.content.len()
        }
    }

    /// Returns the text of the 0-indexed line, consulting the active content
    /// source: pretty JSON, virtual file, or raw content vec.
    pub fn line_text(&self, index: usize) -> Option<&str> {
        if self.is_json && self.show_pretty_json && !self.json_pretty_text.is_empty() {
            self.json_pretty_text.get(index).map(|s| s.as_str())
        } else if let Some(vf) = &self.virtual_file {
            vf.line_text(index)
        } else {
            self.content.get(index).map(|s| s.as_str())
        }
    }

    /// Returns the display width of line `index` in terminal columns.
    pub fn line_width(&self, index: usize) -> Option<usize> {
        if let Some(vf) = &self.virtual_file {
            vf.line_width(index)
        } else {
            self.line_text(index)
                .map(unicode_width::UnicodeWidthStr::width)
        }
    }

    /// Syntax-highlights a slice of lines for the visible window.
    pub fn highlight_lines(
        &self,
        path: &std::path::Path,
        lines: &[&str],
    ) -> Vec<Vec<(ratatui::style::Style, String)>> {
        self.highlighter.highlight_range(path, lines)
    }

    /// Drains all pending worker responses, applying the one matching the most
    /// recent `load_seq` and discarding superseded results. Returns `true` if a
    /// load was applied (so the caller knows to redraw).
    pub(super) fn drain_loads(&mut self) -> bool {
        // Collect first so the immutable borrow of `self.loader` is released
        // before `apply_*` takes `&mut self`.
        let responses: Vec<LoadResponse> =
            std::iter::from_fn(|| self.loader.rx.try_recv().ok()).collect();
        let mut applied = false;
        for resp in responses {
            match resp {
                LoadResponse::File { seq, path, load } => {
                    if seq == self.load_seq {
                        self.apply_file_load(&path, *load);
                        self.loading = false;
                        applied = true;
                    }
                }
                LoadResponse::Diff { seq, path, load } => {
                    if seq == self.load_seq {
                        self.apply_diff_load(&path, *load);
                        self.loading = false;
                        applied = true;
                    }
                }
            }
        }
        applied
    }

    /// Bumps the load sequence so any in-flight worker result is treated as
    /// stale, and clears the in-flight flag. Returns the new sequence number.
    /// Called by every operation that replaces the displayed content.
    pub(super) fn invalidate_pending_load(&mut self) -> u64 {
        self.load_seq = self.load_seq.wrapping_add(1);
        self.loading = false;
        self.load_seq
    }

    /// Dispatches a file open to the background worker (production) or runs it
    /// synchronously (tests, so assertions can observe content immediately).
    pub(super) fn request_open_file(&mut self, path: &std::path::Path) {
        if cfg!(test) {
            self.open_file(path);
        } else {
            let seq = self.invalidate_pending_load();
            self.loading = true;
            self.loader.request(LoadRequest::File {
                seq,
                path: path.to_path_buf(),
            });
        }
    }

    /// Dispatches a working-tree diff to the background worker (production) or
    /// runs it synchronously (tests).
    pub(super) fn request_working_tree_diff(&mut self, path: &std::path::Path) {
        if cfg!(test) {
            self.show_working_tree_diff(path);
        } else {
            let seq = self.invalidate_pending_load();
            self.loading = true;
            self.loader.request(LoadRequest::Diff {
                seq,
                root: self.root.clone(),
                path: path.to_path_buf(),
            });
        }
    }

    /// Opens the currently selected node synchronously. Used at startup so the
    /// first file renders on the first frame.
    pub(super) fn open_selected_sync(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                return;
            }
            let path = node.path.clone();
            if node.deleted {
                self.show_deleted(&path);
            } else if self.git_mode {
                self.show_working_tree_diff(&path);
            } else {
                self.open_file(&path);
            }
        }
    }

    /// Tells the worker to rebuild its highlighter/theme after a theme change.
    pub(super) fn loader_set_theme(&self) {
        self.loader
            .request(LoadRequest::SetTheme(Box::new(self.theme.clone())));
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
