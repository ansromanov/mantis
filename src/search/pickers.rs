//! All remaining fuzzy-picker overlay types extracted from the monolithic
//! `search.rs`. Hosts `ThemePicker`, `RecentFilesState`, `PluginPicker`,
//! `GotoLineState`, `TreeFilter`, and `InFileSearch`/`InFileMatch`, plus
//! their `ListPicker` implementations.

use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// State for the inline tree filter (/ while the tree panel is focused).
///
/// The query is matched case-insensitively against each node's file/directory
/// name; parent directories of matching nodes are also kept so the tree remains
/// navigable. An empty query means no filter is active.
pub struct TreeFilter {
    pub query: String,
    /// Cached visible indices for the current query + tree revision.
    /// `None` when no cache is built yet or the query is empty.
    /// `Some((query, revision, indices))` where revision matches `App::tree_revision`.
    pub(crate) cached: Option<(String, u64, Vec<usize>)>,
}

impl TreeFilter {
    /// Creates a new, empty tree filter.
    pub fn new() -> Self {
        TreeFilter {
            query: String::new(),
            cached: None,
        }
    }

    /// Appends `c` to the query.
    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.cached = None;
    }

    /// Removes the last character from the query.
    pub fn pop(&mut self) {
        self.query.pop();
        self.cached = None;
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

impl ListPicker for TreeFilter {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.is_empty()
    }
    fn results_len(&self) -> usize {
        0
    }
    fn selected(&self) -> usize {
        0
    }
    fn set_selected(&mut self, _i: usize) {}
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

impl ListPicker for GotoLineState {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        0
    }
    fn selected(&self) -> usize {
        0
    }
    fn set_selected(&mut self, _i: usize) {}
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

impl ListPicker for InFileSearch {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.matches.len()
    }
    fn selected(&self) -> usize {
        self.current
    }
    fn set_selected(&mut self, i: usize) {
        self.current = i;
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
        self.filtered = super::fuzzy_refilter(&self.names, &self.matcher, &self.query, |n| {
            std::borrow::Cow::Borrowed(n.as_str())
        });
    }
}

impl ListPicker for ThemePicker {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.results_len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
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
        self.filtered = super::fuzzy_refilter(&self.paths, &self.matcher, &self.query, |p| {
            p.to_string_lossy()
        });
    }
}

impl ListPicker for RecentFilesState {
    fn query_push(&mut self, c: char) {
        self.push(c);
    }
    fn query_pop(&mut self) {
        self.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.results_len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
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

impl ListPicker for PluginPicker {
    fn query_push(&mut self, _c: char) {}
    fn query_pop(&mut self) {}
    fn query_is_empty(&self) -> bool {
        true
    }
    fn results_len(&self) -> usize {
        self.results_len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
    }
}

#[cfg(test)]
#[path = "pickers_test.rs"]
mod tests;
