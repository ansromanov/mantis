use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::config::{pressed, Config, Keymap};
use crate::file::is_binary;
use crate::highlight::Highlighter;
use crate::markdown;
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
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.file_results = scored.into_iter().map(|(p, _)| p).collect();
    }

    fn refresh_content(&mut self) {
        self.content_results = Vec::new();
        if self.query.len() < 2 {
            return;
        }
        let q = self.query.to_lowercase();
        for path in &self.all_files {
            let Ok(text) = fs::read_to_string(path) else { continue };
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
    pub focus: Focus,
    pub search: Option<SearchState>,
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    pub tree_width: u16,
    pub show_help: bool,
    pub should_quit: bool,
    keys: Keymap,
    highlighter: Highlighter,
    last_refresh: Instant,
}

impl App {
    pub fn new(root: PathBuf, cfg: Config) -> anyhow::Result<Self> {
        let expanded = HashSet::new();
        let nodes = build_visible(&root, &expanded, cfg.show_hidden, cfg.ignore_gitignore);
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
            focus: Focus::Tree,
            search: None,
            show_hidden: cfg.show_hidden,
            ignore_gitignore: cfg.ignore_gitignore,
            tree_width: cfg.tree_width,
            show_help: false,
            should_quit: false,
            keys: cfg.keys,
            highlighter: Highlighter::new(),
            last_refresh: Instant::now(),
        };
        app.try_open_selected();
        Ok(app)
    }

    pub fn reload(&mut self) {
        self.last_refresh = Instant::now();
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        if let Some(s) = &mut self.search {
            s.reload_files(&root, show_hidden, ignore_gitignore);
        }
        self.rebuild();
        if let Some(path) = self.current_file.clone() {
            self.open_file(&path);
        }
    }

    pub fn tick(&mut self) {
        if self.last_refresh.elapsed().as_secs() >= 30 {
            self.reload();
        }
    }

    fn rebuild(&mut self) {
        let prev = self.nodes.get(self.tree_selected).map(|n| n.path.clone());
        self.nodes = build_visible(&self.root, &self.expanded, self.show_hidden, self.ignore_gitignore);
        if let Some(p) = prev {
            if let Some(i) = self.nodes.iter().position(|n| n.path == p) {
                self.tree_selected = i;
                return;
            }
        }
        self.tree_selected = self.tree_selected.min(self.nodes.len().saturating_sub(1));
    }

    fn try_open_selected(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if !node.is_dir {
                let path = node.path.clone();
                self.open_file(&path);
            }
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
        self.content_scroll = 0;
        self.content_hscroll = 0;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.is_markdown = matches!(ext, "md" | "markdown");
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();

        if is_binary(path) {
            self.content = vec!["[binary file]".into()];
            self.highlighted = Vec::new();
            return;
        }

        match fs::read_to_string(path) {
            Ok(s) => {
                self.content = s.lines().map(|l| l.to_owned()).collect();
                if self.content.is_empty() {
                    self.content = vec!["[empty file]".into()];
                    self.highlighted = Vec::new();
                } else {
                    self.highlighted = self.highlighter.highlight(path, &self.content);
                    if self.is_markdown {
                        self.markdown_lines = markdown::render(&s);
                    }
                }
            }
            Err(e) => {
                self.content = vec![format!("[error: {}]", e)];
                self.highlighted = Vec::new();
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            if matches!(key.code, KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')) {
                self.show_help = false;
            }
            return;
        }
        if self.search.is_some() {
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
            KeyCode::Enter => {
                let action = self.search.as_ref().and_then(|s| match s.mode {
                    SearchMode::Files => s
                        .file_results
                        .get(s.selected)
                        .map(|p| (p.clone(), None)),
                    SearchMode::Content => s.content_results.get(s.selected).map(|m| {
                        (m.path.clone(), Some(m.line_num))
                    }),
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

    fn handle_normal_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if pressed(&k.quit, &key) {
            self.should_quit = true;
        } else if pressed(&k.help, &key) {
            self.show_help = !self.show_help;
        } else if pressed(&k.toggle_hidden, &key) {
            self.show_hidden = !self.show_hidden;
            self.reload();
        } else if pressed(&k.search_files, &key) {
            let root = self.root.clone();
            self.search = Some(SearchState::new(&root, self.show_hidden, self.ignore_gitignore));
        } else if pressed(&k.reload, &key) {
            self.reload();
        } else if pressed(&k.search_content, &key) {
            let root = self.root.clone();
            let mut s = SearchState::new(&root, self.show_hidden, self.ignore_gitignore);
            s.toggle_mode();
            self.search = Some(s);
        } else if pressed(&k.switch_panel, &key) {
            self.focus = match self.focus {
                Focus::Tree => Focus::Content,
                Focus::Content => Focus::Tree,
            };
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
            if let Some(node) = self.nodes.get(self.tree_selected) {
                if node.is_dir {
                    let p = node.path.clone();
                    if self.expanded.contains(&p) {
                        self.expanded.remove(&p);
                    } else {
                        self.expanded.insert(p);
                    }
                    self.rebuild();
                } else {
                    let p = node.path.clone();
                    self.open_file(&p);
                }
            }
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

    fn handle_content_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if self.is_markdown && pressed(&k.toggle_raw_markdown, &key) {
            self.show_raw_markdown = !self.show_raw_markdown;
            self.content_scroll = 0;
            self.content_hscroll = 0;
        } else if pressed(&k.toggle_wrap, &key) {
            self.word_wrap = !self.word_wrap;
            self.content_scroll = 0;
            self.content_hscroll = 0;
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
            self.content_scroll = self.content.len().saturating_sub(1);
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
