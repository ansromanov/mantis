use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::search::{CommandPalette, HistoryState, InFileSearch, SearchState, ThemePicker};
use crate::selection::TextSelection;
use crate::theme::Theme;
use crate::tree::{build_visible, TreeNode};
use crate::virtual_file::VirtualFile;

mod content_pos;
mod file_ops;
mod key_handlers;
mod mouse_handlers;
mod navigation;

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
    pub content: Vec<String>,
    pub highlighted: Vec<Vec<(ratatui::style::Style, String)>>,
    pub markdown_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    pub virtual_file: Option<VirtualFile>,
    pub is_markdown: bool,
    pub show_raw_markdown: bool,
    pub content_scroll: usize,
    pub content_hscroll: usize,
    pub word_wrap: bool,
    pub current_file: Option<PathBuf>,
    pub is_diff: bool,
    pub content_title: Option<String>,
    pub focus: Focus,
    pub search: Option<SearchState>,
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
    pub selection: Option<TextSelection>,
    drag_start: Option<(usize, usize)>,
    scrollbar_drag: bool,
    /// Set to `true` after suspending the TUI (e.g. for editor), signals
    /// `main.rs` to call `terminal.clear()` before the next `draw()`.
    pub needs_clear: bool,
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
        let mut app = App {
            root,
            nodes,
            expanded,
            tree_selected: 0,
            content: Vec::new(),
            highlighted: Vec::new(),
            markdown_lines: Vec::new(),
            virtual_file: None,
            is_markdown: false,
            show_raw_markdown: false,
            content_scroll: 0,
            content_hscroll: 0,
            word_wrap: cfg.word_wrap,
            current_file: None,
            is_diff: false,
            content_title: None,
            focus: Focus::Tree,
            search: None,
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
            selection: None,
            drag_start: None,
            scrollbar_drag: false,
            needs_clear: false,
        };
        if app.git_mode {
            app.expand_git_dirs();
            app.rebuild();
        }
        app.try_open_selected();
        Ok(app)
    }

    /// Persists the current config to disk if a config path was provided.
    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            config::save(&self.config, path);
        }
    }

    /// Rebuilds the file tree, re-fetches git status, and reloads the current
    /// file. Called explicitly by the reload key and automatically every 30 s.
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

    /// Periodic per-frame update: drains file-watch events and triggers a
    /// periodic full reload every 30 seconds.
    pub fn keys(&self) -> &Keymap {
        &self.keys
    }

    pub fn tick(&mut self) {
        if self.drain_file_watch() {
            self.reload_content();
        }
        if let Some(ref mut s) = self.search {
            s.maybe_refresh();
        }
        if self.last_refresh.elapsed().as_secs() >= 30 {
            self.reload();
        }
    }

    /// Returns the total number of lines in the current content source
    /// (virtual file, raw content, or markdown-rendered lines).
    pub fn line_count(&self) -> usize {
        if self.is_markdown && !self.show_raw_markdown {
            self.markdown_lines.len()
        } else if let Some(vf) = &self.virtual_file {
            vf.line_count()
        } else {
            self.content.len()
        }
    }

    /// Returns the text of the 0-indexed line, consulting the virtual file
    /// first and falling back to the raw content vec.
    pub fn line_text(&self, index: usize) -> Option<&str> {
        if let Some(vf) = &self.virtual_file {
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
mod tests;
