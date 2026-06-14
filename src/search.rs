use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::file::is_binary_bytes;
use crate::tree::collect_all_files;

#[derive(Debug, PartialEq)]
pub enum SearchMode {
    Files,
    Content,
}

pub struct ContentMatch {
    pub path: PathBuf,
    pub line_num: usize,
    pub line: String,
    pub context: Vec<String>,
}

pub struct SearchState {
    pub query: String,
    pub mode: SearchMode,
    all_files: Vec<PathBuf>,
    pub file_results: Vec<PathBuf>,
    pub content_results: Vec<ContentMatch>,
    pub selected: usize,
    matcher: SkimMatcherV2,
    content_cache: HashMap<PathBuf, String>,
    content_cache_dirty: bool,
    pending_refresh: Option<Instant>,
    context_lines: usize,
}

impl SearchState {
    pub fn new(
        root: &Path,
        show_hidden: bool,
        ignore_gitignore: bool,
        context_lines: usize,
    ) -> Self {
        let all_files = collect_all_files(root, show_hidden, ignore_gitignore);
        let file_results = all_files.clone();
        SearchState {
            query: String::new(),
            mode: SearchMode::Files,
            all_files,
            file_results,
            content_results: Vec::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
            content_cache: HashMap::new(),
            content_cache_dirty: true,
            pending_refresh: None,
            context_lines,
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
        match self.mode {
            SearchMode::Files => self.refresh(),
            SearchMode::Content => {
                self.pending_refresh = Some(Instant::now());
            }
        }
    }

    pub fn pop(&mut self) {
        self.query.pop();
        match self.mode {
            SearchMode::Files => self.refresh(),
            SearchMode::Content => {
                self.pending_refresh = Some(Instant::now());
            }
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            SearchMode::Files => SearchMode::Content,
            SearchMode::Content => SearchMode::Files,
        };
        self.selected = 0;
        self.pending_refresh = None;
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
        self.content_cache.clear();
        self.content_cache_dirty = true;
        self.pending_refresh = None;
        self.refresh();
    }

    /// If a debounced content refresh is pending and 100ms have elapsed since
    /// the last keystroke, run it now. Called from every frame tick.
    pub fn maybe_refresh(&mut self) {
        if let Some(t) = self.pending_refresh {
            if t.elapsed() >= std::time::Duration::from_millis(100) {
                self.pending_refresh = None;
                self.refresh();
            }
        }
    }

    /// Force an immediate refresh, bypassing any debounce.
    pub fn refresh_now(&mut self) {
        self.pending_refresh = None;
        self.refresh();
    }

    fn refresh_files(&mut self) {
        if self.query.is_empty() {
            self.file_results = self.all_files.clone();
            return;
        }
        let mut scored: Vec<(PathBuf, i64)> = self
            .all_files
            .iter()
            .filter_map(|p| {
                self.matcher
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
        // Build cache from disk if dirty (first call or after tree reload).
        if self.content_cache_dirty {
            self.content_cache.clear();
            for path in &self.all_files {
                let Ok(bytes) = fs::read(path) else { continue };
                if is_binary_bytes(&bytes) {
                    continue;
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                self.content_cache.insert(path.clone(), text);
            }
            self.content_cache_dirty = false;
        }
        let q = self.query.to_lowercase();
        let ctx = self.context_lines;
        for path in &self.all_files {
            let Some(text) = self.content_cache.get(path) else {
                continue;
            };
            let all_lines: Vec<&str> = text.lines().collect();
            for (i, line) in all_lines.iter().enumerate() {
                if line.to_lowercase().contains(&q) {
                    let context = if ctx > 0 {
                        let end = (i + 1 + ctx).min(all_lines.len());
                        all_lines[i + 1..end]
                            .iter()
                            .map(|l| l.to_string())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    self.content_results.push(ContentMatch {
                        path: path.clone(),
                        line_num: i + 1,
                        line: line.to_string(),
                        context,
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
    matcher: SkimMatcherV2,
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
            matcher: SkimMatcherV2::default(),
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
        let mut scored: Vec<(usize, i64)> = self
            .commits
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let hay = format!("{} {} {}", c.short, c.date, c.subject);
                self.matcher
                    .fuzzy_match(&hay, &self.query)
                    .map(|sc| (i, sc))
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

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }

    /// Re-computes matches by calling `get_line(i)` for each 0-based line index
    /// up to `line_count`. The caller provides the line provider (which may read
    /// from a virtual file or from an in-memory vec).
    pub fn refresh(&mut self, line_count: usize, get_line: impl Fn(usize) -> Option<String>) {
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
        for i in 0..line_count {
            let Some(line) = get_line(i) else { continue };
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

/// Fuzzy-filterable list of discovered themes.
pub struct ThemePicker {
    pub names: Vec<String>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    matcher: SkimMatcherV2,
}

impl Default for ThemePicker {
    fn default() -> Self {
        let names: Vec<String> = crate::theme::Theme::discover_all()
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        let filtered = (0..names.len()).collect();
        ThemePicker {
            names,
            query: String::new(),
            filtered,
            selected: 0,
            matcher: SkimMatcherV2::default(),
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

    pub fn selected_name(&self) -> Option<&str> {
        self.filtered
            .get(self.selected)
            .map(|&i| self.names[i].as_str())
    }

    fn refilter(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.filtered = (0..self.names.len()).collect();
            return;
        }
        let mut scored: Vec<(usize, i64)> = self
            .names
            .iter()
            .enumerate()
            .filter_map(|(i, n)| self.matcher.fuzzy_match(n, &self.query).map(|s| (i, s)))
            .collect();
        scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
}

pub use crate::command_palette::CommandPalette;

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::AtomicUsize;
    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn search_temp_dir(label: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("tv_search_{}_{}_{}", label, std::process::id(), n))
    }

    // -- InFileSearch ----------------------------------------------------------

    #[test]
    fn in_file_search_finds_matches() {
        let mut s = InFileSearch::new();
        assert!(s.matches.is_empty());
        assert_eq!(s.current, 0);

        let lines = ["hello world".to_string(), "foo bar".to_string()];
        s.push('o');
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        // 'o' matches at positions 4, 7 in "hello world" and 1, 2 in "foo bar"
        assert_eq!(s.matches.len(), 4);

        s.push(' ');
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        // "o " matches at position 4 in "hello world" and position 1 in "foo bar"
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.matches[0].line, 0);
        assert_eq!(s.matches[0].col, 4);

        s.pop();
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        assert_eq!(s.matches.len(), 4);
    }

    #[test]
    fn in_file_search_case_insensitive() {
        let mut s = InFileSearch::new();
        let lines = ["Hello World".to_string()];
        s.push('w');
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        assert_eq!(s.matches.len(), 1);
        assert_eq!(s.matches[0].col, 6);
    }

    #[test]
    fn in_file_search_empty_query_clears_matches() {
        let mut s = InFileSearch::new();
        let lines = ["hello".to_string()];
        s.push('h');
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        assert_eq!(s.matches.len(), 1);
        s.pop();
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        assert!(s.matches.is_empty());
    }

    #[test]
    fn in_file_search_current_navigation() {
        let mut s = InFileSearch::new();
        let lines = ["aa".to_string()];
        s.push('a');
        s.refresh(lines.len(), |i| lines.get(i).cloned());
        assert_eq!(s.matches.len(), 2);
        assert_eq!(s.current, 0);

        // Can't directly test next/prev as they live in key_handlers,
        // but we verify push sets current to 0.
    }

    #[test]
    fn in_file_search_default_is_empty() {
        let s = InFileSearch::default();
        assert!(s.query.is_empty());
        assert!(s.matches.is_empty());
        assert_eq!(s.current, 0);
    }

    // -- ThemePicker -----------------------------------------------------------

    #[test]
    fn theme_picker_starts_with_all_presets() {
        let p = ThemePicker::default();
        let count = crate::theme::Theme::discover_all().len();
        assert_eq!(p.names.len(), count);
        assert_eq!(p.filtered.len(), count);
        assert_eq!(p.selected, 0);
    }

    #[test]
    fn theme_picker_push_filters() {
        let mut p = ThemePicker::default();
        p.push('m');
        assert!(p.filtered.len() < p.names.len());
        assert!(p.filtered.iter().any(|&i| p.names[i].contains("monokai")));
    }

    #[test]
    fn theme_picker_pop_restores() {
        let mut p = ThemePicker::default();
        p.push('m');
        let filtered_after_push = p.filtered.len();
        p.pop();
        assert_eq!(p.filtered.len(), p.names.len());
        assert!(filtered_after_push < p.names.len());
    }

    #[test]
    fn theme_picker_selected_name() {
        let mut p = ThemePicker::default();
        p.push('m');
        let name = p.selected_name();
        assert!(name.is_some());
        assert!(name.unwrap().contains("monokai"));
    }

    #[test]
    fn theme_picker_selected_name_returns_none_when_empty() {
        let mut p = ThemePicker::default();
        // Filter with a query that matches nothing
        for c in "zzzzzzz".chars() {
            p.push(c);
        }
        assert_eq!(p.results_len(), 0);
        assert!(p.selected_name().is_none());
    }

    #[test]
    fn theme_picker_results_len() {
        let p = ThemePicker::default();
        assert_eq!(p.results_len(), crate::theme::Theme::discover_all().len());
    }

    // -- HistoryState ----------------------------------------------------------

    fn sample_commits() -> Vec<crate::git::Commit> {
        vec![
            crate::git::Commit {
                hash: "abc123def456".into(),
                short: "abc123".into(),
                date: "2024-01-15".into(),
                subject: "fix critical bug".into(),
            },
            crate::git::Commit {
                hash: "def789abc012".into(),
                short: "def789".into(),
                date: "2024-01-14".into(),
                subject: "add new feature".into(),
            },
            crate::git::Commit {
                hash: "ghi345jkl678".into(),
                short: "ghi345".into(),
                date: "2024-01-13".into(),
                subject: "refactor module".into(),
            },
        ]
    }

    #[test]
    fn history_state_starts_with_all_commits() {
        let commits = sample_commits();
        let h = HistoryState::new(PathBuf::from("f.txt"), commits);
        assert_eq!(h.results_len(), 3);
        assert_eq!(h.selected, 0);
    }

    #[test]
    fn history_state_push_filters() {
        let commits = sample_commits();
        let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
        h.push('b');
        assert!(h.results_len() < 3);
        assert_eq!(h.filtered[0], 0); // "fix critical bug" scores highest
    }

    #[test]
    fn history_state_pop_restores() {
        let commits = sample_commits();
        let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
        h.push('b');
        let after_push = h.results_len();
        h.pop();
        assert_eq!(h.results_len(), 3);
        assert!(after_push < 3);
    }

    #[test]
    fn history_state_selected_commit() {
        let commits = sample_commits();
        let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
        assert_eq!(h.selected_commit().unwrap().short, "abc123");
        h.selected = 1;
        assert_eq!(h.selected_commit().unwrap().short, "def789");
    }

    #[test]
    fn history_state_selected_commit_returns_none_out_of_bounds() {
        let commits = sample_commits();
        let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
        h.selected = 99;
        assert!(h.selected_commit().is_none());
    }

    #[test]
    fn history_state_filtered_out_of_bounds() {
        let commits = sample_commits();
        // Filter until empty
        let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
        for c in "zzzzzzz".chars() {
            h.push(c);
        }
        assert_eq!(h.results_len(), 0);
        assert!(h.selected_commit().is_none());
    }

    // -- SearchState -----------------------------------------------------------

    #[test]
    fn search_state_new_creates_file_results() {
        let root = search_temp_dir("new");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "hello\n").unwrap();
        fs::write(root.join("b.txt"), "world\n").unwrap();

        let s = SearchState::new(&root, false, true, 0);
        assert_eq!(s.file_results.len(), 2);
        assert_eq!(s.mode, SearchMode::Files);
        assert!(s.query.is_empty());
        assert_eq!(s.selected, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_push_and_pop_query() {
        let root = search_temp_dir("push_pop");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "hello\n").unwrap();
        fs::write(root.join("b.txt"), "world\n").unwrap();

        let mut s = SearchState::new(&root, false, true, 0);
        assert_eq!(s.file_results.len(), 2);
        s.push('A');
        assert_eq!(s.query, "A");
        s.pop();
        assert_eq!(s.query, "");
        assert_eq!(s.file_results.len(), 2);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_toggle_mode() {
        let root = search_temp_dir("toggle");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "hello\n").unwrap();

        let mut s = SearchState::new(&root, false, true, 0);
        assert_eq!(s.mode, SearchMode::Files);
        s.toggle_mode();
        assert_eq!(s.mode, SearchMode::Content);
        s.toggle_mode();
        assert_eq!(s.mode, SearchMode::Files);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_results_len() {
        let root = search_temp_dir("results_len");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "hello\n").unwrap();

        let mut s = SearchState::new(&root, false, true, 0);
        assert_eq!(s.results_len(), 1);
        s.toggle_mode();
        // Content mode needs 2+ chars
        s.push('h');
        assert_eq!(s.results_len(), 0);
        s.push('e');
        s.refresh_now(); // bypass debounce in test
        assert_eq!(s.results_len(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_content_context_lines() {
        let root = search_temp_dir("context");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("a.yaml"),
            "database:\n  host: db.internal\n  port: 5432\n",
        )
        .unwrap();

        let mut s = SearchState::new(&root, false, true, 2);
        s.toggle_mode();
        s.push('d');
        s.push('a');
        s.push('t');
        s.refresh_now();
        assert_eq!(s.content_results.len(), 1);
        assert_eq!(s.content_results[0].context.len(), 2);
        assert!(s.content_results[0].context[0].contains("host"));
        assert!(s.content_results[0].context[1].contains("port"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_content_context_capped_at_eof() {
        let root = search_temp_dir("context_eof");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "match\nnext\n").unwrap();

        let mut s = SearchState::new(&root, false, true, 5);
        s.toggle_mode();
        for c in "mat".chars() {
            s.push(c);
        }
        s.refresh_now();
        assert_eq!(s.content_results.len(), 1);
        assert_eq!(s.content_results[0].context.len(), 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_state_reload_files() {
        let root = search_temp_dir("reload");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.txt"), "hello\n").unwrap();

        let mut s = SearchState::new(&root, false, true, 0);
        assert_eq!(s.file_results.len(), 1);

        // Add a file and reload
        fs::write(root.join("b.txt"), "world\n").unwrap();
        s.reload_files(&root, false, true);
        assert_eq!(s.file_results.len(), 2);
        fs::remove_dir_all(&root).ok();
    }
}
