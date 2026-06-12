use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use arboard::Clipboard;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use ratatui::layout::Rect;

use crate::config::{self, pressed, Config, Keymap};
use crate::file::is_binary_bytes;
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::markdown;
use crate::theme::{Theme, ThemeConfig};
use crate::tree::{build_visible, collect_all_files, TreeNode};

pub enum Focus {
    Tree,
    Content,
}

#[derive(PartialEq)]
pub enum SearchMode {
    Files,
    Content,
}

pub struct ContentMatch {
    pub path: PathBuf,
    pub line_num: usize,
    pub line: String,
}

pub struct SearchState {
    pub query: String,
    pub mode: SearchMode,
    all_files: Vec<PathBuf>,
    pub file_results: Vec<PathBuf>,
    pub content_results: Vec<ContentMatch>,
    pub selected: usize,
}

impl SearchState {
    pub fn new(root: &Path, show_hidden: bool, ignore_gitignore: bool) -> Self {
        let all_files = collect_all_files(root, show_hidden, ignore_gitignore);
        let file_results = all_files.clone();
        SearchState {
            query: String::new(),
            mode: SearchMode::Files,
            all_files,
            file_results,
            content_results: Vec::new(),
            selected: 0,
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.refresh();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refresh();
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Files => SearchMode::Content,
            SearchMode::Content => SearchMode::Files,
        };
        self.selected = 0;
        self.refresh();
    }

    pub fn results_len(&self) -> usize {
        match self.mode {
            SearchMode::Files => self.file_results.len(),
            SearchMode::Content => self.content_results.len(),
        }
    }

    fn refresh(&mut self) {
        self.selected = 0;
        match self.mode {
            SearchMode::Files => self.refresh_files(),
            SearchMode::Content => self.refresh_content(),
        }
    }

    pub fn reload_files(&mut self, root: &Path, show_hidden: bool, ignore_gitignore: bool) {
        self.all_files = collect_all_files(root, show_hidden, ignore_gitignore);
        self.refresh();
    }

    fn refresh_files(&mut self) {
        if self.query.is_empty() {
            self.file_results = self.all_files.clone();
            return;
        }
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(PathBuf, i64)> = self
            .all_files
            .iter()
            .filter_map(|p| {
                matcher
                    .fuzzy_match(&p.to_string_lossy(), &self.query)
                    .map(|sc| (p.clone(), sc))
            })
            .collect();
        scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
        self.file_results = scored.into_iter().map(|(p, _)| p).collect();
    }

    fn refresh_content(&mut self) {
        self.content_results = Vec::new();
        if self.query.len() < 2 {
            return;
        }
        let q = self.query.to_lowercase();
        for path in &self.all_files {
            let Ok(bytes) = fs::read(path) else {
                continue;
            };
            // Skip binary blobs rather than scanning them line by line.
            if is_binary_bytes(&bytes) {
                continue;
            }
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            for (i, line) in text.lines().enumerate() {
                if line.to_lowercase().contains(&q) {
                    self.content_results.push(ContentMatch {
                        path: path.clone(),
                        line_num: i + 1,
                        line: line.to_owned(),
                    });
                }
            }
        }
    }
}

/// Fuzzy-filterable list of the commits that touched a single file.
pub struct HistoryState {
    pub file: PathBuf,
    pub commits: Vec<crate::git::Commit>,
    pub query: String,
    pub filtered: Vec<usize>, // indices into `commits`, in display order
    pub selected: usize,
}

impl HistoryState {
    pub fn new(file: PathBuf, commits: Vec<crate::git::Commit>) -> Self {
        let filtered = (0..commits.len()).collect();
        HistoryState {
            file,
            commits,
            query: String::new(),
            filtered,
            selected: 0,
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn results_len(&self) -> usize {
        self.filtered.len()
    }

    pub fn selected_commit(&self) -> Option<&crate::git::Commit> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.commits.get(i))
    }

    fn refilter(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.filtered = (0..self.commits.len()).collect();
            return;
        }
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(usize, i64)> = self
            .commits
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let hay = format!("{} {} {}", c.short, c.date, c.subject);
                matcher.fuzzy_match(&hay, &self.query).map(|sc| (i, sc))
            })
            .collect();
        scored.sort_by_key(|(_, sc)| std::cmp::Reverse(*sc));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
}

/// Fuzzy-filterable list of built-in theme presets.
pub struct ThemePicker {
    pub names: Vec<&'static str>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
}

impl Default for ThemePicker {
    fn default() -> Self {
        let names = crate::theme::PRESETS.to_vec();
        let filtered = (0..names.len()).collect();
        ThemePicker {
            names,
            query: String::new(),
            filtered,
            selected: 0,
        }
    }
}

impl ThemePicker {
    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    pub fn results_len(&self) -> usize {
        self.filtered.len()
    }

    pub fn selected_name(&self) -> Option<&'static str> {
        self.filtered.get(self.selected).map(|&i| self.names[i])
    }

    fn refilter(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.filtered = (0..self.names.len()).collect();
            return;
        }
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(usize, i64)> = self
            .names
            .iter()
            .enumerate()
            .filter_map(|(i, n)| matcher.fuzzy_match(n, &self.query).map(|s| (i, s)))
            .collect();
        scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
}

pub struct TextSelection {
    pub anchor: (usize, usize),
    pub active: (usize, usize),
}

impl TextSelection {
    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.active {
            (self.anchor, self.active)
        } else {
            (self.active, self.anchor)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.active
    }
}

pub struct App {
    pub root: PathBuf,
    pub nodes: Vec<TreeNode>,
    pub expanded: HashSet<PathBuf>,
    pub tree_selected: usize,
    pub content: Vec<String>,
    pub highlighted: Vec<Vec<(ratatui::style::Style, String)>>,
    pub markdown_lines: Vec<Vec<(ratatui::style::Style, String)>>,
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
    pub git_status_map: HashMap<PathBuf, GitStatus>,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    keys: Keymap,
    config: Config,
    config_path: Option<std::path::PathBuf>,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
    pub content_area: Rect,
    pub search_area: Rect,
    pub search_offset: usize,
    pub history_area: Rect,
    pub history_offset: usize,
    pub theme_area: Rect,
    pub theme_offset: usize,
    // Time and result index of the last search-result click, for double-click.
    last_click: Option<(Instant, usize)>,
    highlighter: Highlighter,
    last_refresh: Instant,
    file_watcher: Option<RecommendedWatcher>,
    file_watch_rx: Option<Receiver<notify::Result<notify::Event>>>,
    file_watch_path: Option<PathBuf>,
    pub selection: Option<TextSelection>,
    drag_start: Option<(usize, usize)>,
}

impl App {
    pub fn new(
        root: PathBuf,
        cfg: Config,
        config_path: Option<std::path::PathBuf>,
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
        let deleted = deleted_set(&git_status_map, git_show_deleted);
        let nodes = build_visible(
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
            git_status_map,
            git_mode: cfg.git_mode,
            git_mode_flat: cfg.git_mode_flat,
            keys: cfg.keys,
            config: saved_config,
            config_path,
            tree_area: Rect::default(),
            tree_offset: 0,
            content_area: Rect::default(),
            search_area: Rect::default(),
            search_offset: 0,
            history_area: Rect::default(),
            history_offset: 0,
            theme_area: Rect::default(),
            theme_offset: 0,
            last_click: None,
            highlighter,
            last_refresh: Instant::now(),
            file_watcher: None,
            file_watch_rx: None,
            file_watch_path: None,
            selection: None,
            drag_start: None,
        };
        if app.git_mode {
            app.expand_git_dirs();
            app.rebuild();
        }
        app.try_open_selected();
        Ok(app)
    }

    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            config::save(&self.config, path);
        }
    }

    pub fn reload(&mut self) {
        self.last_refresh = Instant::now();
        if self.git_status_enabled {
            self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
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

    fn reload_content(&mut self) {
        // Commit diffs are transient; don't clobber them on refresh.
        // Working-tree diffs in git mode should be refreshed (working tree changes).
        if self.is_diff && !self.git_mode {
            return;
        }
        if let Some(path) = self.current_file.clone() {
            let scroll = self.content_scroll;
            let hscroll = self.content_hscroll;
            if self.git_mode {
                self.show_working_tree_diff(&path);
            } else {
                let raw = self.show_raw_markdown;
                self.open_file(&path);
                self.show_raw_markdown = raw;
            }
            self.content_scroll = scroll.min(self.content_line_count().saturating_sub(1));
            self.content_hscroll = hscroll;
        }
    }

    fn set_file_watch(&mut self, path: Option<&Path>) {
        self.file_watcher = None;
        self.file_watch_rx = None;
        self.file_watch_path = None;
        let Some(p) = path else { return };
        // Watch the parent directory rather than the file itself so that
        // atomic-save editors (those that write a temp file and rename it over
        // the original) still trigger events after the inode is replaced.
        let Some(dir) = p.parent() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            self.file_watcher = Some(watcher);
            self.file_watch_rx = Some(rx);
            self.file_watch_path = Some(p.to_path_buf());
        }
    }

    fn drain_file_watch(&self) -> bool {
        let (Some(rx), Some(watched)) = (&self.file_watch_rx, &self.file_watch_path) else {
            return false;
        };
        let mut changed = false;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                let affects_watched = evt.paths.iter().any(|p| p == watched);
                if affects_watched
                    && matches!(
                        evt.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    )
                {
                    changed = true;
                }
            }
        }
        changed
    }

    pub fn tick(&mut self) {
        if self.drain_file_watch() {
            self.reload_content();
        }
        if self.last_refresh.elapsed().as_secs() >= 30 {
            self.reload();
        }
    }

    fn rebuild(&mut self) {
        let prev = self.nodes.get(self.tree_selected).map(|n| n.path.clone());
        let deleted = deleted_set(&self.git_status_map, self.git_show_deleted);

        if self.git_mode {
            if self.git_mode_flat {
                self.nodes = self.build_git_flat_nodes();
            } else {
                let all = build_visible(
                    &self.root,
                    &self.expanded,
                    self.show_hidden,
                    self.ignore_gitignore,
                    &deleted,
                );
                let map = &self.git_status_map;
                self.nodes = all
                    .into_iter()
                    .filter(|n| {
                        n.deleted || map.get(&n.path).is_some_and(|&s| s != GitStatus::Ignored)
                    })
                    .collect();
            }
        } else {
            self.nodes = build_visible(
                &self.root,
                &self.expanded,
                self.show_hidden,
                self.ignore_gitignore,
                &deleted,
            );
        }

        if let Some(p) = prev {
            if let Some(i) = self.nodes.iter().position(|n| n.path == p) {
                self.tree_selected = i;
                return;
            }
        }
        self.tree_selected = self.tree_selected.min(self.nodes.len().saturating_sub(1));
    }

    /// Flat list of all changed (non-ignored) files for git mode's plain view.
    fn build_git_flat_nodes(&self) -> Vec<TreeNode> {
        let mut entries: Vec<(PathBuf, bool)> = self
            .git_status_map
            .iter()
            .filter(|(path, &status)| {
                status != GitStatus::Ignored && path.starts_with(&self.root) && !path.is_dir()
            })
            .map(|(path, &status)| {
                let deleted = status == GitStatus::Deleted && !path.exists();
                (path.clone(), deleted)
            })
            .collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        entries
            .into_iter()
            .map(|(path, deleted)| {
                let name = path
                    .strip_prefix(&self.root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                TreeNode {
                    path,
                    name,
                    depth: 0,
                    is_dir: false,
                    deleted,
                }
            })
            .collect()
    }

    /// Expands all directories that contain git changes so they are visible in
    /// git mode's tree view.
    fn expand_git_dirs(&mut self) {
        let dirs: Vec<PathBuf> = self
            .git_status_map
            .iter()
            .filter(|(path, &status)| {
                status != GitStatus::Ignored
                    && path.is_dir()
                    && path.starts_with(&self.root)
                    && **path != self.root
            })
            .map(|(p, _)| p.clone())
            .collect();
        for dir in dirs {
            self.expanded.insert(dir);
        }
    }

    fn try_open_selected(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                return;
            }
            if node.deleted {
                let path = node.path.clone();
                self.show_deleted(&path);
            } else if self.git_mode {
                let path = node.path.clone();
                self.show_working_tree_diff(&path);
            } else {
                let path = node.path.clone();
                self.open_file(&path);
            }
        }
    }

    fn show_working_tree_diff(&mut self, path: &Path) {
        let lines = crate::git::working_tree_diff(&self.root, path);
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        self.current_file = Some(path.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_diff = true;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        self.content_title = Some(format!(" working diff — {} ", rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.content = lines;
        self.set_file_watch(Some(path));
    }

    fn toggle_git_mode(&mut self) {
        self.git_mode = !self.git_mode;
        self.config.git_mode = self.git_mode;
        if self.git_mode {
            // Ensure git status is populated even if git_status was disabled.
            if !self.git_status_enabled {
                self.git_status_enabled = true;
                self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
            }
            self.expand_git_dirs();
            self.rebuild();
            self.try_open_selected();
        } else {
            self.rebuild();
            // Re-open the current file as normal content instead of a diff.
            if let Some(path) = self.current_file.clone() {
                if self.is_diff {
                    self.open_file(&path);
                }
            }
        }
        self.save_config();
    }

    fn show_deleted(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
        self.is_diff = false;
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.content = vec!["[deleted]".into()];
        self.highlighted = Vec::new();
        self.markdown_lines = Vec::new();
        self.content_title = None;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        self.set_file_watch(None);
    }

    /// Acts on the currently selected node: toggles a directory's fold state,
    /// or opens a file. Shared by the Enter key and a mouse click.
    fn activate_selected(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                let p = node.path.clone();
                if self.expanded.contains(&p) {
                    self.expanded.remove(&p);
                } else {
                    self.expanded.insert(p);
                }
                self.rebuild();
            } else if node.deleted {
                let p = node.path.clone();
                self.show_deleted(&p);
            } else if self.git_mode {
                let p = node.path.clone();
                self.show_working_tree_diff(&p);
            } else {
                let p = node.path.clone();
                self.open_file(&p);
            }
        }
    }

    /// Opens the currently selected search result and closes the overlay.
    /// Shared by the Enter key and a mouse click in the results list.
    fn activate_search_selection(&mut self) {
        let action = self.search.as_ref().and_then(|s| match s.mode {
            SearchMode::Files => s.file_results.get(s.selected).map(|p| (p.clone(), None)),
            SearchMode::Content => s
                .content_results
                .get(s.selected)
                .map(|m| (m.path.clone(), Some(m.line_num))),
        });
        self.search = None;
        if let Some((path, line)) = action {
            self.open_file(&path);
            if let Some(ln) = line {
                self.content_scroll = ln.saturating_sub(1);
            }
            self.reveal_in_tree(&path.clone());
        }
    }

    fn handle_history_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.history_area, ev.column, ev.row) {
                    return;
                }
                let index = self.history_offset + (ev.row - self.history_area.y) as usize;
                let in_range = self
                    .history
                    .as_ref()
                    .is_some_and(|h| index < h.results_len());
                if !in_range {
                    return;
                }
                if let Some(h) = &mut self.history {
                    h.selected = index;
                }
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.show_selected_revision();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(h) = &mut self.history {
                    if h.selected + 1 < h.results_len() {
                        h.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(h) = &mut self.history {
                    h.selected = h.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    fn handle_theme_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.theme_area, ev.column, ev.row) {
                    return;
                }
                let index = self.theme_offset + (ev.row - self.theme_area.y) as usize;
                let in_range = self
                    .theme_picker
                    .as_ref()
                    .is_some_and(|p| index < p.results_len());
                if !in_range {
                    return;
                }
                if let Some(p) = &mut self.theme_picker {
                    p.selected = index;
                }
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.apply_selected_theme();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(p) = &mut self.theme_picker {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(p) = &mut self.theme_picker {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        if self.show_help {
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_mouse(ev);
            return;
        }
        if self.history.is_some() {
            self.handle_history_mouse(ev);
            return;
        }
        if self.search.is_some() {
            self.handle_search_mouse(ev);
            return;
        }
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
                    self.clear_selection();
                    let row = (ev.row - self.tree_area.y) as usize;
                    let index = self.tree_offset + row;
                    if index < self.nodes.len() {
                        self.tree_selected = index;
                        self.activate_selected();
                    }
                } else if rect_contains(self.content_area, ev.column, ev.row) {
                    self.focus = Focus::Content;
                    let can_select = !(self.is_diff
                        || self.word_wrap
                        || self.is_markdown && !self.show_raw_markdown);
                    if can_select {
                        let pos = self.content_pos(ev.column, ev.row);
                        self.drag_start = Some(pos);
                        self.selection = None;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(start) = self.drag_start {
                    // Auto-scroll before computing position so that selection.active
                    // is calculated with the already-updated scroll offset.
                    let ca = self.content_area;
                    if ev.row < ca.y + 2 {
                        self.content_scroll = self.content_scroll.saturating_sub(1);
                    } else if ev.row >= ca.y + ca.height.saturating_sub(2) {
                        let max = self.content_line_count().saturating_sub(1);
                        self.content_scroll = (self.content_scroll + 1).min(max);
                    }
                    let pos = self.content_pos(ev.column, ev.row);
                    self.selection = Some(TextSelection {
                        anchor: start,
                        active: pos,
                    });
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag_start.is_some() {
                    if let Some(sel) = &self.selection {
                        if !sel.is_empty() {
                            let text = self.selection_text();
                            if !text.is_empty() {
                                if let Ok(mut cb) = Clipboard::new() {
                                    let _ = cb.set_text(text);
                                }
                            }
                        }
                    }
                    self.drag_start = None;
                }
            }
            MouseEventKind::ScrollDown => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    let max = self.content_line_count().saturating_sub(1);
                    self.content_scroll = (self.content_scroll + 3).min(max);
                } else if rect_contains(self.tree_area, ev.column, ev.row)
                    && self.tree_selected + 1 < self.nodes.len()
                {
                    self.tree_selected += 1;
                    self.try_open_selected();
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    self.content_scroll = self.content_scroll.saturating_sub(3);
                } else if rect_contains(self.tree_area, ev.column, ev.row) && self.tree_selected > 0
                {
                    self.tree_selected -= 1;
                    self.try_open_selected();
                }
            }
            _ => {}
        }
    }

    fn handle_search_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.search_area, ev.column, ev.row) {
                    return;
                }
                let index = self.search_offset + (ev.row - self.search_area.y) as usize;
                let in_range = self
                    .search
                    .as_ref()
                    .is_some_and(|s| index < s.results_len());
                if !in_range {
                    return;
                }
                if let Some(s) = &mut self.search {
                    s.selected = index;
                }
                // A second click on the same row within the window opens it.
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.activate_search_selection();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(s) = &mut self.search {
                    if s.selected + 1 < s.results_len() {
                        s.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(s) = &mut self.search {
                    s.selected = s.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    /// Opens a file and selects it in the tree, expanding parent directories
    /// as needed. Used when a file path is passed on the command line.
    pub fn open_and_reveal(&mut self, path: &Path) {
        if !path.exists() && self.git_status_map.get(path) == Some(&GitStatus::Deleted) {
            self.show_deleted(path);
        } else {
            self.open_file(path);
        }
        self.reveal_in_tree(path);
        self.focus = Focus::Content;
    }

    pub fn open_file(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
        self.is_diff = false;
        self.content_title = None;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.is_markdown = matches!(ext, "md" | "markdown");
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();

        // Read the file once: classify it as binary and decode it from the
        // same bytes, rather than reading the whole file twice.
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.content = vec![format!("[error: {}]", e)];
                self.highlighted = Vec::new();
                return;
            }
        };
        if is_binary_bytes(&bytes) {
            self.content = vec!["[binary file]".into()];
            self.highlighted = Vec::new();
            return;
        }
        let s = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                self.content = vec!["[binary file]".into()];
                self.highlighted = Vec::new();
                return;
            }
        };
        self.content = s.lines().map(|l| l.to_owned()).collect();
        if self.content.is_empty() {
            self.content = vec!["[empty file]".into()];
            self.highlighted = Vec::new();
        } else {
            self.highlighted = self.highlighter.highlight(path, &self.content);
            if self.is_markdown {
                self.markdown_lines = markdown::render(&s, &self.theme);
            }
        }
        self.set_file_watch(Some(path));
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            if matches!(
                key.code,
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
            ) {
                self.show_help = false;
            }
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_key(key);
        } else if self.history.is_some() {
            self.handle_history_key(key);
        } else if self.search.is_some() {
            self.handle_search_key(key);
        } else {
            self.handle_normal_key(key);
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.search = None;
            }
            KeyCode::Tab => {
                if let Some(s) = &mut self.search {
                    s.toggle_mode();
                }
            }
            KeyCode::Enter => self.activate_search_selection(),
            KeyCode::Up => {
                if let Some(s) = &mut self.search {
                    s.selected = s.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(s) = &mut self.search {
                    if s.selected + 1 < s.results_len() {
                        s.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(s) = &mut self.search {
                    s.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(s) = &mut self.search {
                    s.push(c);
                }
            }
            _ => {}
        }
    }

    /// Opens the git history of the currently displayed file as a picker.
    /// Does nothing if no file is open or the file has no tracked history.
    fn open_file_history(&mut self) {
        let Some(file) = self.current_file.clone() else {
            return;
        };
        let commits = crate::git::file_log(&self.root, &file);
        if commits.is_empty() {
            return;
        }
        self.history = Some(HistoryState::new(file, commits));
    }

    fn handle_history_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.history = None,
            KeyCode::Enter => self.show_selected_revision(),
            KeyCode::Up => {
                if let Some(h) = &mut self.history {
                    h.selected = h.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(h) = &mut self.history {
                    if h.selected + 1 < h.results_len() {
                        h.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(h) = &mut self.history {
                    h.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(h) = &mut self.history {
                    h.push(c);
                }
            }
            _ => {}
        }
    }

    /// Loads the diff of the selected revision into the content panel.
    fn show_selected_revision(&mut self) {
        let picked = self.history.as_ref().and_then(|h| {
            h.selected_commit()
                .map(|c| (c.hash.clone(), c.short.clone(), h.file.clone()))
        });
        self.history = None;
        if let Some((hash, short, file)) = picked {
            let diff = crate::git::file_diff(&self.root, &hash, &file);
            self.show_diff(&file, &short, diff);
        }
    }

    fn show_diff(&mut self, file: &Path, short: &str, lines: Vec<String>) {
        self.current_file = Some(file.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_diff = true;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        self.content_title = Some(format!(" diff {} — {} ", short, rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.content = lines;
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }

    fn handle_theme_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.theme_picker = None,
            KeyCode::Enter => self.apply_selected_theme(),
            KeyCode::Up => {
                if let Some(p) = &mut self.theme_picker {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(p) = &mut self.theme_picker {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = &mut self.theme_picker {
                    p.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = &mut self.theme_picker {
                    p.push(c);
                }
            }
            _ => {}
        }
    }

    fn apply_selected_theme(&mut self) {
        let name = self.theme_picker.as_ref().and_then(|p| p.selected_name());
        self.theme_picker = None;
        if let Some(name) = name {
            if let Some(theme) = Theme::preset(name) {
                self.apply_theme(theme);
                self.config.theme = ThemeConfig::from_preset(name);
                self.save_config();
            }
        }
    }

    /// Switches the active theme and re-renders the current view with it,
    /// preserving scroll position.
    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.highlighter = Highlighter::new(&self.theme.syntax);
        if self.is_diff {
            self.highlighted = self
                .content
                .iter()
                .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
                .collect();
        } else if let Some(path) = self.current_file.clone() {
            let scroll = self.content_scroll;
            let hscroll = self.content_hscroll;
            let raw = self.show_raw_markdown;
            self.open_file(&path);
            self.show_raw_markdown = raw;
            self.content_scroll = scroll.min(self.content_line_count().saturating_sub(1));
            self.content_hscroll = hscroll;
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc && self.selection.is_some() {
            self.clear_selection();
            return;
        }
        let k = &self.keys;
        if pressed(&k.quit, &key) {
            self.should_quit = true;
        } else if pressed(&k.help, &key) {
            self.show_help = !self.show_help;
        } else if pressed(&k.toggle_hidden, &key) {
            self.show_hidden = !self.show_hidden;
            self.config.show_hidden = self.show_hidden;
            self.reload();
            self.save_config();
        } else if pressed(&k.search_files, &key) {
            let root = self.root.clone();
            self.search = Some(SearchState::new(
                &root,
                self.show_hidden,
                self.ignore_gitignore,
            ));
        } else if pressed(&k.reload, &key) {
            self.reload();
        } else if pressed(&k.search_content, &key) {
            let root = self.root.clone();
            let mut s = SearchState::new(&root, self.show_hidden, self.ignore_gitignore);
            s.toggle_mode();
            self.search = Some(s);
        } else if pressed(&k.file_history, &key) {
            self.open_file_history();
        } else if pressed(&k.theme_picker, &key) {
            self.theme_picker = Some(ThemePicker::default());
        } else if pressed(&k.switch_panel, &key) {
            self.focus = match self.focus {
                Focus::Tree => Focus::Content,
                Focus::Content => Focus::Tree,
            };
        } else if pressed(&k.git_mode_toggle, &key) {
            self.toggle_git_mode();
        } else if pressed(&k.git_mode_flat_toggle, &key) {
            if self.git_mode {
                self.git_mode_flat = !self.git_mode_flat;
                self.config.git_mode_flat = self.git_mode_flat;
                self.rebuild();
                self.try_open_selected();
                self.save_config();
            }
        } else {
            match self.focus {
                Focus::Tree => self.handle_tree_key(key),
                Focus::Content => self.handle_content_key(key),
            }
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if pressed(&k.nav_up, &key) {
            if self.tree_selected > 0 {
                self.tree_selected -= 1;
                self.try_open_selected();
            }
        } else if pressed(&k.nav_down, &key) {
            if self.tree_selected + 1 < self.nodes.len() {
                self.tree_selected += 1;
                self.try_open_selected();
            }
        } else if pressed(&k.tree_expand, &key) {
            self.activate_selected();
        } else if pressed(&k.tree_collapse, &key) {
            if let Some(node) = self.nodes.get(self.tree_selected) {
                let depth = node.depth;
                let path = node.path.clone();
                let is_dir = node.is_dir;

                if is_dir && self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                    self.rebuild();
                } else if depth > 0 {
                    for i in (0..self.tree_selected).rev() {
                        if self.nodes[i].depth < depth {
                            self.tree_selected = i;
                            break;
                        }
                    }
                }
            }
        }
    }

    fn content_line_count(&self) -> usize {
        if self.is_markdown && !self.show_raw_markdown {
            self.markdown_lines.len()
        } else {
            self.content.len()
        }
    }

    /// Width of the line-number gutter (digits + space), or 0 when there is none.
    pub fn line_prefix_width(&self) -> usize {
        if self.is_diff || (self.is_markdown && !self.show_raw_markdown) {
            0
        } else {
            self.content.len().to_string().len().max(1) + 1
        }
    }

    /// Convert a terminal cell inside `content_area` to a `(buffer_line, buffer_col)` position.
    pub fn content_pos(&self, col: u16, row: u16) -> (usize, usize) {
        let ca = self.content_area;
        let rel_row = (row.saturating_sub(ca.y)) as usize;
        let rel_col = (col.saturating_sub(ca.x)) as usize;
        let buf_line = self.content_scroll + rel_row;
        let prefix = self.line_prefix_width();
        let buf_col = (rel_col + self.content_hscroll).saturating_sub(prefix);
        (buf_line, buf_col)
    }

    /// Extract the currently selected text from `self.content`.
    pub fn selection_text(&self) -> String {
        let Some(sel) = &self.selection else {
            return String::new();
        };
        if sel.is_empty() {
            return String::new();
        }
        let ((start_line, start_col), (end_line, end_col)) = sel.normalized();
        let lines = &self.content;
        if start_line >= lines.len() {
            return String::new();
        }
        let mut result = String::new();
        let last = end_line.min(lines.len().saturating_sub(1));
        for (line_idx, line) in lines
            .iter()
            .enumerate()
            .skip(start_line)
            .take(last - start_line + 1)
        {
            let chars: Vec<char> = line.chars().collect();
            let col_start = if line_idx == start_line { start_col } else { 0 };
            let col_end = if line_idx == end_line {
                end_col.min(chars.len())
            } else {
                chars.len()
            };
            if !result.is_empty() {
                result.push('\n');
            }
            result.extend(&chars[col_start.min(chars.len())..col_end]);
        }
        result
    }

    fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_start = None;
    }

    fn handle_content_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if self.is_markdown && pressed(&k.toggle_raw_markdown, &key) {
            self.show_raw_markdown = !self.show_raw_markdown;
            self.content_scroll = 0;
            self.content_hscroll = 0;
        } else if pressed(&k.toggle_wrap, &key) {
            self.word_wrap = !self.word_wrap;
            self.config.word_wrap = self.word_wrap;
            self.content_scroll = 0;
            self.content_hscroll = 0;
            self.save_config();
        } else if pressed(&k.nav_up, &key) {
            self.content_scroll = self.content_scroll.saturating_sub(1);
        } else if pressed(&k.nav_down, &key) {
            let max = self.content_line_count().saturating_sub(1);
            if self.content_scroll < max {
                self.content_scroll += 1;
            }
        } else if pressed(&k.content_page_up, &key) {
            self.content_scroll = self.content_scroll.saturating_sub(20);
        } else if pressed(&k.content_page_down, &key) {
            let max = self.content_line_count().saturating_sub(1);
            self.content_scroll = (self.content_scroll + 20).min(max);
        } else if !self.word_wrap && pressed(&k.content_left, &key) {
            self.content_hscroll = self.content_hscroll.saturating_sub(4);
        } else if !self.word_wrap && pressed(&k.content_right, &key) {
            self.content_hscroll += 4;
        } else if pressed(&k.content_top, &key) {
            self.content_scroll = 0;
        } else if pressed(&k.content_bottom, &key) {
            self.content_scroll = self.content_line_count().saturating_sub(1);
        } else if !self.word_wrap && pressed(&k.content_reset_col, &key) {
            self.content_hscroll = 0;
        }
    }

    fn reveal_in_tree(&mut self, path: &Path) {
        let mut current = path.parent();
        while let Some(dir) = current {
            if dir == self.root {
                break;
            }
            if dir.starts_with(&self.root) {
                self.expanded.insert(dir.to_path_buf());
            } else {
                break;
            }
            current = dir.parent();
        }
        self.rebuild();
        if let Some(i) = self.nodes.iter().position(|n| n.path == path) {
            self.tree_selected = i;
        }
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
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Creates a temp directory tree:
    ///   sub/ (with c.txt), a.txt, b.txt, long.txt (50 lines)
    fn temp_tree() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_app_test_{}_{n}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
        fs::write(dir.join("b.txt"), "hello\n").unwrap();
        fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
        let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
        fs::write(dir.join("long.txt"), long).unwrap();
        dir.canonicalize().unwrap()
    }

    fn app_for(root: &Path) -> App {
        App::new(root.to_path_buf(), Config::default(), None).unwrap()
    }

    /// A temp git repo with one committed file plus an uncommitted change.
    fn temp_git_tree() -> PathBuf {
        use std::process::Command;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_app_git_{}_{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q"]);
        fs::write(dir.join("tracked.txt"), "one\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&["commit", "-q", "-m", "add tracked"]);
        fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn file_history_opens_picker_and_shows_diff() {
        let root = temp_git_tree();
        let mut app = app_for(&root);
        app.open_file(&root.join("tracked.txt"));

        // H opens the history picker.
        app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
        assert!(app.history.is_some());
        assert!(!app.history.as_ref().unwrap().commits.is_empty());

        // Enter loads the diff into the content panel.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.history.is_none());
        assert!(app.is_diff);
        assert!(app.content_title.is_some());
        assert!(app.content.iter().any(|l| l.starts_with("+two")));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn open_and_reveal_selects_file_in_tree() {
        let root = temp_tree();
        let mut app = app_for(&root);
        // Reveal a file nested inside a collapsed subdirectory.
        let nested = root.join("sub").join("c.txt");
        app.open_and_reveal(&nested);

        assert_eq!(app.current_file.as_deref(), Some(nested.as_path()));
        assert!(matches!(app.focus, Focus::Content));
        // The parent dir is expanded and the file node is selected.
        assert!(app.expanded.contains(&root.join("sub")));
        assert_eq!(
            app.nodes.get(app.tree_selected).map(|n| n.path.clone()),
            Some(nested)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn file_history_noop_without_git_history() {
        let root = temp_tree(); // not a git repo
        let mut app = app_for(&root);
        app.open_file(&root.join("a.txt"));
        app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
        assert!(app.history.is_none());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn theme_picker_applies_preset() {
        let root = temp_tree();
        let mut app = app_for(&root);
        assert_eq!(app.theme.accent, crate::theme::Theme::default().accent);

        // `t` opens the picker.
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
        assert!(app.theme_picker.is_some());

        // Filter to "monokai" and apply it.
        for c in "monokai".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert!(app.theme_picker.is_none());
        assert_eq!(
            app.theme.accent,
            crate::theme::Theme::preset("monokai").unwrap().accent
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn theme_picker_esc_cancels() {
        let root = temp_tree();
        let mut app = app_for(&root);
        let before = app.theme.accent;
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.theme_picker.is_none());
        assert_eq!(app.theme.accent, before); // unchanged
        fs::remove_dir_all(&root).ok();
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn click(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Down(MouseButton::Left), col, row)
    }

    fn full_rect() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }
    }

    #[test]
    fn rect_contains_checks_bounds() {
        let r = Rect {
            x: 2,
            y: 3,
            width: 4,
            height: 5,
        };
        assert!(rect_contains(r, 2, 3)); // top-left corner
        assert!(rect_contains(r, 5, 7)); // inside, near far corner
        assert!(!rect_contains(r, 6, 3)); // column == x + width
        assert!(!rect_contains(r, 2, 8)); // row == y + height
        assert!(!rect_contains(r, 1, 3)); // left of area
        assert!(!rect_contains(r, 2, 2)); // above area
    }

    #[test]
    fn left_click_in_tree_opens_file() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;
        app.focus = Focus::Content;

        let idx = app.nodes.iter().position(|n| !n.is_dir).unwrap();
        let path = app.nodes[idx].path.clone();
        app.handle_mouse(click(1, idx as u16));

        assert_eq!(app.tree_selected, idx);
        assert_eq!(app.current_file.as_deref(), Some(path.as_path()));
        assert!(matches!(app.focus, Focus::Tree));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn left_click_on_dir_toggles_expand() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;

        let dir_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
        let dir_path = app.nodes[dir_idx].path.clone();
        let before = app.nodes.len();

        app.handle_mouse(click(1, dir_idx as u16));
        assert!(app.expanded.contains(&dir_path));
        assert!(app.nodes.len() > before, "child should become visible");

        app.handle_mouse(click(1, dir_idx as u16));
        assert!(!app.expanded.contains(&dir_path));
        assert_eq!(app.nodes.len(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn left_click_respects_scroll_offset() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 1; // first visible row is node index 1

        app.handle_mouse(click(1, 0));
        assert_eq!(app.tree_selected, 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn click_below_last_node_is_ignored() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;
        app.tree_selected = 0;

        // Row far past the last node.
        app.handle_mouse(click(1, 18));
        assert_eq!(app.tree_selected, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scroll_wheel_moves_tree_selection() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.content_area = Rect {
            x: 100,
            y: 0,
            width: 40,
            height: 20,
        };
        app.tree_selected = 0;

        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.tree_selected, 1);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.tree_selected, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scroll_wheel_scrolls_content() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.open_file(&root.join("long.txt"));
        app.content_area = full_rect();
        app.tree_area = Rect {
            x: 100,
            y: 0,
            width: 40,
            height: 20,
        };

        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.content_scroll, 3);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.content_scroll, 0);
        fs::remove_dir_all(&root).ok();
    }

    fn open_file_search(app: &mut App) {
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        assert!(app.search.is_some());
        app.search_area = full_rect();
        app.search_offset = 0;
    }

    #[test]
    fn search_single_click_selects_without_opening() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);

        app.handle_mouse(click(1, 1));
        assert_eq!(app.search.as_ref().unwrap().selected, 1);
        assert!(app.search.is_some(), "single click should not open");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_double_click_opens_result() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);

        let target = app.search.as_ref().unwrap().file_results[0].clone();
        app.handle_mouse(click(1, 0));
        app.handle_mouse(click(1, 0)); // second click, same row, within window

        assert!(
            app.search.is_none(),
            "double click should open and close search"
        );
        assert_eq!(app.current_file.as_deref(), Some(target.as_path()));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_click_on_different_row_does_not_open() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);
        // Need at least two results for this to be meaningful.
        if app.search.as_ref().unwrap().results_len() >= 2 {
            app.handle_mouse(click(1, 0));
            app.handle_mouse(click(1, 1));
            assert!(
                app.search.is_some(),
                "clicks on different rows must not open"
            );
            assert_eq!(app.search.as_ref().unwrap().selected, 1);
        }
        fs::remove_dir_all(&root).ok();
    }

    // ── git mode ─────────────────────────────────────────────────────────────

    /// Repo with:
    ///   committed.txt  – committed "original", working-tree modified to "modified"
    ///   unchanged.txt  – committed "stable", untouched (must stay invisible in git mode)
    ///   new.txt        – untracked
    ///   sub/nested.txt – committed "nested", working-tree modified (gives sub/ a status)
    fn temp_git_with_changes() -> PathBuf {
        use std::process::Command;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_git_mode_{}_{n}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q"]);
        fs::write(dir.join("committed.txt"), "original\n").unwrap();
        fs::write(dir.join("unchanged.txt"), "stable\n").unwrap();
        fs::write(dir.join("sub").join("nested.txt"), "nested\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);
        fs::write(dir.join("committed.txt"), "modified\n").unwrap();
        fs::write(dir.join("sub").join("nested.txt"), "nested modified\n").unwrap();
        fs::write(dir.join("new.txt"), "brand new\n").unwrap();
        dir.canonicalize().unwrap()
    }

    fn ctrl_g() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)
    }

    fn alt_g() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT)
    }

    #[test]
    fn git_mode_filters_tree_to_changed_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        assert!(app.git_mode);
        let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
        // Changed items must appear.
        assert!(names.contains(&"committed.txt"), "nodes: {names:?}");
        assert!(names.contains(&"new.txt"), "nodes: {names:?}");
        // Unchanged file must be absent.
        assert!(!names.contains(&"unchanged.txt"), "nodes: {names:?}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_toggle_off_restores_unchanged_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g()); // on
        app.handle_key(ctrl_g()); // off

        assert!(!app.git_mode);
        assert!(!app.is_diff, "should restore file content view");
        assert!(
            app.nodes.iter().any(|n| n.name == "unchanged.txt"),
            "unchanged file must reappear after exiting git mode"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_auto_expands_dirs_with_changes() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);
        assert!(
            !app.expanded.contains(&root.join("sub")),
            "sub/ starts collapsed"
        );

        app.handle_key(ctrl_g());

        assert!(
            app.expanded.contains(&root.join("sub")),
            "git mode must auto-expand dirs containing changes"
        );
        assert!(
            app.nodes.iter().any(|n| n.path.ends_with("nested.txt")),
            "nested changed file must be visible in git mode tree"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_opens_working_tree_diff() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        // Navigate past any leading directory nodes to land on a file.
        // (tree.rs sorts dirs first, so sub/ may be at index 0.)
        let file_idx = app
            .nodes
            .iter()
            .position(|n| !n.is_dir)
            .expect("git mode must have at least one file node");
        for _ in 0..file_idx {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
        }

        assert!(
            app.is_diff,
            "selecting a file in git mode must show working-tree diff"
        );
        assert!(
            app.content_title
                .as_deref()
                .unwrap_or("")
                .contains("working diff"),
            "title was {:?}",
            app.content_title
        );
        assert!(
            app.content.iter().any(|l| l.starts_with('+')),
            "diff must contain added lines"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_navigation_shows_diff_for_each_file() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        // Move to the next file node.
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));

        assert!(
            app.is_diff,
            "navigation in git mode must keep showing diffs"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_shows_depth_zero_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        app.handle_key(alt_g());

        assert!(app.git_mode_flat);
        assert!(
            app.nodes.iter().all(|n| n.depth == 0 && !n.is_dir),
            "flat mode must only have depth-0 file nodes"
        );
        // Root-level file appears as bare name; nested file as relative path.
        assert!(app.nodes.iter().any(|n| n.name == "committed.txt"));
        assert!(app.nodes.iter().any(|n| n.name.contains("nested.txt")));
        // Unchanged file still absent.
        assert!(!app.nodes.iter().any(|n| n.name.contains("unchanged.txt")));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_toggle_returns_to_tree_view() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        app.handle_key(alt_g()); // flat
        app.handle_key(alt_g()); // back to tree

        assert!(app.git_mode);
        assert!(!app.git_mode_flat);
        assert!(
            app.nodes.iter().any(|n| n.is_dir),
            "tree view should include directory nodes"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_key_is_noop_outside_git_mode() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);
        let count = app.nodes.len();

        app.handle_key(alt_g());

        assert!(!app.git_mode_flat);
        assert!(!app.git_mode);
        assert_eq!(app.nodes.len(), count, "tree must be unchanged");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_outside_repo_gives_empty_tree() {
        let root = temp_tree(); // not a git repo
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        assert!(app.git_mode);
        assert!(
            app.nodes.is_empty(),
            "no git changes → empty filtered tree; got {} nodes",
            app.nodes.len()
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_config_starts_enabled() {
        let root = temp_git_with_changes();
        let cfg = Config {
            git_mode: true,
            ..Config::default()
        };
        let app = App::new(root.to_path_buf(), cfg, None).unwrap();

        assert!(app.git_mode);
        assert!(
            !app.nodes.iter().any(|n| n.name == "unchanged.txt"),
            "unchanged file must be absent when starting in git mode"
        );
        assert!(
            app.nodes.iter().any(|n| n.name == "committed.txt"),
            "changed file must be visible when starting in git mode"
        );
        fs::remove_dir_all(&root).ok();
    }
}
