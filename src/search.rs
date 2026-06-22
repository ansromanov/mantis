//! Fuzzy file and content search, plus the shared fuzzy-picker overlays.
//!
//! Provides the search engine and the state behind several look-alike overlays.
//! `SearchState` drives the main picker, which fuzzy-matches either file paths
//! (`SearchMode::Files`) or file contents (`SearchMode::Content`) using
//! `SkimMatcherV2`, debouncing the expensive content scans. `ContentMatch`
//! carries a hit's path, line number, and surrounding context. The same
//! query/filtered-list/selected-index shape backs `HistoryState`, `ThemePicker`,
//! `CommandPalette`, `RecentFilesState`, `PluginPicker`, the in-file search (`InFileSearch`),
//! and the go-to-line dialog (`GotoLineState`), all defined here. Results sort by
//! descending fuzzy score; binary files are skipped via `is_binary_bytes`.

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
    // Lines stored as (original, lowercased) so query refreshes need no per-line allocation.
    content_cache: HashMap<PathBuf, Vec<(String, String)>>,
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
        // Use char count so a single multibyte character doesn't bypass the guard.
        if self.query.chars().count() < 2 {
            return;
        }
        // Build cache from disk if dirty (first call or after tree reload).
        // Each file is pre-split into (original, lowercased) line pairs so that
        // query refreshes need no per-line allocation.
        if self.content_cache_dirty {
            self.content_cache.clear();
            for path in &self.all_files {
                let Ok(bytes) = fs::read(path) else { continue };
                // Skip files larger than 1 MB to cap memory use.
                if bytes.len() > 1024 * 1024 {
                    continue;
                }
                if is_binary_bytes(&bytes) {
                    continue;
                }
                let Ok(text) = String::from_utf8(bytes) else {
                    continue;
                };
                let lines: Vec<(String, String)> = text
                    .lines()
                    .map(|l| (l.to_string(), l.to_lowercase()))
                    .collect();
                self.content_cache.insert(path.clone(), lines);
            }
            self.content_cache_dirty = false;
        }
        let q = self.query.to_lowercase();
        let ctx = self.context_lines;
        for path in &self.all_files {
            let Some(lines) = self.content_cache.get(path) else {
                continue;
            };
            for (i, (orig, lower)) in lines.iter().enumerate() {
                if lower.contains(&q) {
                    let context = if ctx > 0 {
                        let end = (i + 1 + ctx).min(lines.len());
                        lines[i + 1..end].iter().map(|(l, _)| l.clone()).collect()
                    } else {
                        Vec::new()
                    };
                    self.content_results.push(ContentMatch {
                        path: path.clone(),
                        line_num: i + 1,
                        line: orig.clone(),
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

/// State for the inline tree filter (/ while the tree panel is focused).
///
/// The query is matched case-insensitively against each node's file/directory
/// name; parent directories of matching nodes are also kept so the tree remains
/// navigable. An empty query means no filter is active.
pub struct TreeFilter {
    pub query: String,
}

impl TreeFilter {
    /// Creates a new, empty tree filter.
    pub fn new() -> Self {
        TreeFilter {
            query: String::new(),
        }
    }

    /// Appends `c` to the query.
    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    /// Removes the last character from the query.
    pub fn pop(&mut self) {
        self.query.pop();
    }

    /// Returns `true` when the query is empty (no filter applied).
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

impl Default for TreeFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// State for the go-to-line dialog.
///
/// Opened by the `goto_line` keybinding (default `:`). The user types a line
/// number and presses Enter to jump (1-indexed, clamped to valid range), or Esc
/// to cancel. Supports relative jumps: `+N` jumps forward N lines, `-N` jumps
/// backward N lines.
pub struct GotoLineState {
    pub query: String,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self::new()
    }
}

impl GotoLineState {
    pub fn new() -> Self {
        GotoLineState {
            query: String::new(),
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
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

/// Fuzzy-filterable list of recently opened files, most-recent-first.
pub struct RecentFilesState {
    pub paths: Vec<PathBuf>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    matcher: SkimMatcherV2,
}

impl RecentFilesState {
    /// Creates the state from a snapshot of the ring buffer (already in
    /// most-recent-first order, current file excluded by the caller).
    pub fn new(paths: Vec<PathBuf>) -> Self {
        let filtered = (0..paths.len()).collect();
        RecentFilesState {
            paths,
            query: String::new(),
            filtered,
            selected: 0,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Pushes a character onto the query and refilters.
    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.refilter();
    }

    /// Removes the last character from the query and refilters.
    pub fn pop(&mut self) {
        self.query.pop();
        self.refilter();
    }

    /// Returns the number of filtered results.
    pub fn results_len(&self) -> usize {
        self.filtered.len()
    }

    /// Returns the path of the currently selected entry, if any.
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.paths.get(i))
    }

    fn refilter(&mut self) {
        self.selected = 0;
        if self.query.is_empty() {
            self.filtered = (0..self.paths.len()).collect();
            return;
        }
        let mut scored: Vec<(usize, i64)> = self
            .paths
            .iter()
            .enumerate()
            .filter_map(|(i, p)| {
                self.matcher
                    .fuzzy_match(&p.to_string_lossy(), &self.query)
                    .map(|sc| (i, sc))
            })
            .collect();
        scored.sort_by_key(|(_, sc)| std::cmp::Reverse(*sc));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
    }
}

/// Scrollable list of registered plugins with their running state for the plugin manager overlay.
pub struct PluginPicker {
    /// `(name, is_running, kind)` for each registered plugin, in the order provided
    /// by the manager (alphabetical by name as loaded from config).
    pub entries: Vec<(String, bool, crate::plugin::PluginKind)>,
    pub selected: usize,
}

impl PluginPicker {
    pub fn new(entries: Vec<(String, bool, crate::plugin::PluginKind)>) -> Self {
        PluginPicker {
            entries,
            selected: 0,
        }
    }

    pub fn results_len(&self) -> usize {
        self.entries.len()
    }
}

pub use crate::command_palette::CommandPalette;

#[cfg(test)]
#[path = "search_test.rs"]
mod tests;
