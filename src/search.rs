use std::fs;
use std::path::{Path, PathBuf};

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::file::is_binary_bytes;
use crate::tree::collect_all_files;

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
    pub filtered: Vec<usize>,
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

/// A single match occurrence within a file for in-file search.
#[derive(Clone, Debug)]
pub struct InFileMatch {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// State for in-file search (/ while content panel is focused).
pub struct InFileSearch {
    pub query: String,
    pub matches: Vec<InFileMatch>,
    pub current: usize,
}

impl Default for InFileSearch {
    fn default() -> Self {
        Self::new()
    }
}

impl InFileSearch {
    pub fn new() -> Self {
        InFileSearch {
            query: String::new(),
            matches: Vec::new(),
            current: 0,
        }
    }

    pub fn push(&mut self, c: char, lines: &[String]) {
        self.query.push(c);
        self.refresh(lines);
    }

    pub fn pop(&mut self, lines: &[String]) {
        self.query.pop();
        self.refresh(lines);
    }

    fn refresh(&mut self, lines: &[String]) {
        self.matches.clear();
        self.current = 0;
        if self.query.is_empty() {
            return;
        }
        let q_lower: Vec<char> = self.query.to_lowercase().chars().collect();
        let q_char_len = q_lower.len();
        if q_char_len == 0 {
            return;
        }
        for (i, line) in lines.iter().enumerate() {
            let line_lower: Vec<char> = line.to_lowercase().chars().collect();
            if line_lower.len() < q_char_len {
                continue;
            }
            for start in 0..=line_lower.len() - q_char_len {
                if line_lower[start..start + q_char_len] == q_lower[..] {
                    self.matches.push(InFileMatch {
                        line: i,
                        col: start,
                        len: q_char_len,
                    });
                }
            }
        }
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
