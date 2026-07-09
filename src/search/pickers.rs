//! All remaining fuzzy-picker overlay types extracted from the monolithic
//! `search.rs`. Hosts `ThemePicker`, `RecentFilesState`, `PluginPicker`,
//! `GotoLineState`, `TreeFilter`, and `InFileSearch`/`InFileMatch`, plus
//! their `ListPicker` implementations.

use std::collections::HashSet;
use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// State for the inline tree filter (/ while the tree panel is focused).
///
/// The query is matched case-insensitively against each node's file/directory
/// name; parent directories of matching nodes are also kept so the tree remains
/// navigable. An empty query means no filter is active.
///
/// The query is interpreted as a regex if it compiles; otherwise it falls back
/// to plain substring matching so incomplete/invalid patterns don't crash or
/// blank the tree while typing.
pub struct TreeFilter {
    pub query: String,
    /// Compiled regex for the current query, if the query is a valid regex.
    /// `None` when the query is empty or is not a valid regex (substring fallback).
    pub(crate) regex: Option<regex::Regex>,
    /// Lowercased copy of `query`, kept in sync in `rebuild_regex` so the
    /// substring-fallback path in `matches_name` doesn't re-lowercase the
    /// query on every node it's called against.
    lowercase_query: String,
    /// Cached visible indices for the current query + tree revision.
    /// `None` when no cache is built yet or the query is empty.
    /// `Some((query, revision, indices))` where revision matches `App::tree_revision`.
    pub(crate) cached: Option<(String, u64, Vec<usize>)>,
    /// Snapshot of `App::expanded` taken just before the filter auto-expanded
    /// any directories, so it can be restored once the filter is dismissed.
    /// `None` until the first non-empty query triggers an auto-expansion.
    pub(crate) saved_expanded: Option<HashSet<PathBuf>>,
    /// Every directory and file path under the tree root, paired with its
    /// lowercased file name, used to match against the query regardless of
    /// current expansion state. Built lazily on the first non-empty query and
    /// reused for the rest of the filter session so keystrokes neither re-walk
    /// the filesystem nor re-lowercase (re-allocate) every name.
    pub(crate) full_paths_cache: Option<Vec<(PathBuf, String)>>,
}

impl TreeFilter {
    /// Creates a new, empty tree filter.
    pub fn new() -> Self {
        TreeFilter {
            query: String::new(),
            regex: None,
            lowercase_query: String::new(),
            cached: None,
            saved_expanded: None,
            full_paths_cache: None,
        }
    }

    /// Rebuilds the compiled regex (or clears it) and the lowercased query
    /// cache to reflect the current query.
    fn rebuild_regex(&mut self) {
        self.regex = if self.query.is_empty() {
            None
        } else {
            crate::search::build_search_regex(&self.query, true, false, false)
        };
        self.lowercase_query = self.query.to_lowercase();
    }

    /// Appends `c` to the query.
    pub fn push(&mut self, c: char) {
        self.query.push(c);
        self.cached = None;
        self.rebuild_regex();
    }

    /// Removes the last character from the query.
    pub fn pop(&mut self) {
        self.query.pop();
        self.cached = None;
        self.rebuild_regex();
    }

    /// Returns `true` when the query is empty (no filter applied).
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    /// Returns `true` if `name` matches the current filter.
    ///
    /// When the query compiles as a valid regex, matching is done via the regex
    /// (case-insensitive). Otherwise it falls back to case-insensitive substring
    /// containment, so incomplete or invalid regex patterns don't break filtering
    /// — they degrade gracefully to literal matching.
    pub fn matches_name(&self, name: &str) -> bool {
        if self.query.is_empty() {
            return true;
        }
        match &self.regex {
            Some(re) => re.is_match(name),
            None => name.to_lowercase().contains(&self.lowercase_query),
        }
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

/// State for the compare-against dialog.
///
/// Opened from the command palette. The user types a revision (e.g. `HEAD~3`,
/// a commit hash, or a branch name) and presses Enter to enter compare mode
/// against that revision, or Esc to cancel.
pub struct CompareModeInput {
    pub query: String,
}

impl Default for CompareModeInput {
    fn default() -> Self {
        Self::new()
    }
}

impl CompareModeInput {
    pub fn new() -> Self {
        CompareModeInput {
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

impl ListPicker for CompareModeInput {
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
    /// Interpret the query as a regular expression (Ctrl+R).
    pub regex: bool,
    /// Match case-sensitively instead of the default insensitive (Ctrl+A).
    pub case_sensitive: bool,
    /// Match whole words only (Ctrl+W).
    pub whole_word: bool,
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
            regex: false,
            case_sensitive: false,
            whole_word: false,
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

        if self.regex || self.whole_word {
            let Some(re) = super::build_search_regex(
                &self.query,
                self.regex,
                self.whole_word,
                self.case_sensitive,
            ) else {
                return;
            };
            for i in 0..line_count {
                let Some(line) = get_line(i) else { continue };
                for mat in re.find_iter(&line) {
                    // A pattern like `x*` matches the empty string at every
                    // position; zero-length matches are useless as highlights.
                    if mat.start() == mat.end() {
                        continue;
                    }
                    let char_start = line[..mat.start()].chars().count();
                    let char_len = line[mat.start()..mat.end()].chars().count();
                    self.matches.push(InFileMatch {
                        line: i,
                        col: char_start,
                        len: char_len,
                    });
                }
            }
        } else {
            let q = if self.case_sensitive {
                self.query.clone()
            } else {
                self.query.to_lowercase()
            };
            let q_chars: Vec<char> = q.chars().collect();
            let q_char_len = q_chars.len();
            if q_char_len == 0 {
                return;
            }
            for i in 0..line_count {
                let Some(line) = get_line(i) else { continue };
                let target = if self.case_sensitive {
                    line
                } else {
                    line.to_lowercase()
                };
                let target_chars: Vec<char> = target.chars().collect();
                if target_chars.len() < q_char_len {
                    continue;
                }
                for start in 0..=target_chars.len() - q_char_len {
                    if target_chars[start..start + q_char_len] == q_chars[..] {
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
///
/// `themes` mirrors `names` and holds the already-parsed `Theme` for each
/// entry (loaded once by `discover_all` at construction) so navigating the
/// list for live preview doesn't re-read/re-parse theme files from disk on
/// every keystroke.
pub struct ThemePicker {
    pub names: Vec<String>,
    pub themes: Vec<crate::theme::Theme>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    matcher: SkimMatcherV2,
}

impl Default for ThemePicker {
    fn default() -> Self {
        let (names, themes): (Vec<String>, Vec<crate::theme::Theme>) =
            crate::theme::Theme::discover_all().into_iter().unzip();
        let filtered = (0..names.len()).collect();
        ThemePicker {
            names,
            themes,
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

    /// The already-parsed theme for the current selection, avoiding a disk
    /// re-read for live preview.
    pub fn selected_theme(&self) -> Option<&crate::theme::Theme> {
        self.filtered.get(self.selected).map(|&i| &self.themes[i])
    }

    fn refilter(&mut self) {
        self.selected = 0;
        self.filtered = super::fuzzy_refilter(
            &self.names,
            &self.matcher,
            &self.query,
            |n| std::borrow::Cow::Borrowed(n.as_str()),
            false,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect();
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
        self.filtered = super::fuzzy_refilter(
            &self.paths,
            &self.matcher,
            &self.query,
            |p| p.to_string_lossy(),
            false,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect();
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
    /// `(name, is_running, kind, crash_badge)` for each registered plugin, in
    /// the order provided by the manager (alphabetical by name as loaded from
    /// config). `crash_badge` is `Some(summary)` when the plugin isn't running
    /// and its last run exited unexpectedly.
    pub entries: Vec<(String, bool, crate::plugin::PluginKind, Option<String>)>,
    pub selected: usize,
}

impl PluginPicker {
    pub fn new(entries: Vec<(String, bool, crate::plugin::PluginKind, Option<String>)>) -> Self {
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

/// State for the bug report modal.
/// Holds the typed text lines, the cursor position, and the viewport scroll offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugReportState {
    pub text: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_top: usize,
    pub preview_scroll: crate::scroll::ScrollState,
    pub diagnostics_markdown: String,
}

impl Default for BugReportState {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl BugReportState {
    pub fn new(diagnostics_markdown: String) -> Self {
        BugReportState {
            text: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_top: 0,
            preview_scroll: crate::scroll::ScrollState::new(),
            diagnostics_markdown,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.text.is_empty() {
            self.text.push(String::new());
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col > char_len {
            self.cursor_col = char_len;
        }
        let byte_idx = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        line.insert(byte_idx, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        if self.text.is_empty() {
            self.text.push(String::new());
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col > char_len {
            self.cursor_col = char_len;
        }
        let byte_idx = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        let next_line = line.split_off(byte_idx);
        self.text.insert(self.cursor_row + 1, next_line);
        self.cursor_row += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.text.is_empty() {
            return;
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        if self.cursor_col > 0 {
            let line = &mut self.text[self.cursor_row];
            let char_len = line.chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
            let char_idx = self.cursor_col - 1;
            let byte_idx = line
                .char_indices()
                .map(|(i, _)| i)
                .nth(char_idx)
                .unwrap_or(line.len());
            line.remove(byte_idx);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let current_line = self.text.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_line = &mut self.text[self.cursor_row];
            let prev_len = prev_line.chars().count();
            prev_line.push_str(&current_line);
            self.cursor_col = prev_len;
        }
    }

    pub fn delete(&mut self) {
        if self.text.is_empty() {
            return;
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col < char_len {
            let byte_idx = line
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.cursor_col)
                .unwrap_or(line.len());
            line.remove(byte_idx);
        } else if self.cursor_row + 1 < self.text.len() {
            let next_line = self.text.remove(self.cursor_row + 1);
            let line = &mut self.text[self.cursor_row];
            line.push_str(&next_line);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.text[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let char_len = self.text[self.cursor_row].chars().count();
        if self.cursor_col < char_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.text.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let char_len = self.text[self.cursor_row].chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.text.len() {
            self.cursor_row += 1;
            let char_len = self.text[self.cursor_row].chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        if !self.text.is_empty() {
            self.cursor_col = self.text[self.cursor_row].chars().count();
        }
    }

    fn visual_row_count(line: &str, width: usize) -> usize {
        if width == 0 {
            return 1;
        }
        let n = line.chars().count();
        if n == 0 {
            1
        } else {
            n.div_ceil(width)
        }
    }

    /// Visual (rendered) row index of the cursor for the given character-cell edit width.
    pub fn cursor_visual_row(&self, width: usize) -> usize {
        let mut visual = 0;
        for (i, line) in self.text.iter().enumerate() {
            if i == self.cursor_row {
                return visual + self.cursor_col / width.max(1);
            }
            visual += Self::visual_row_count(line, width);
        }
        visual
    }

    /// Total visual (rendered) rows across all logical lines for the given edit width.
    pub fn total_visual_rows(&self, width: usize) -> usize {
        let mut total = 0;
        for (i, line) in self.text.iter().enumerate() {
            let n = line.chars().count();
            let mut rows = Self::visual_row_count(line, width);
            if i == self.cursor_row && self.cursor_col == n && n > 0 && n % width.max(1) == 0 {
                rows += 1;
            }
            total += rows;
        }
        total
    }

    pub fn clamp_scroll(&mut self, height: usize, width: usize) {
        if height == 0 || width == 0 {
            return;
        }
        let cursor_vis = self.cursor_visual_row(width);
        if cursor_vis < self.scroll_top {
            self.scroll_top = cursor_vis;
        } else if cursor_vis >= self.scroll_top + height {
            self.scroll_top = cursor_vis - height + 1;
        }
        let total_vis = self.total_visual_rows(width);
        let max_scroll = total_vis.saturating_sub(height);
        if self.scroll_top > max_scroll {
            self.scroll_top = max_scroll;
        }
    }
}

#[cfg(test)]
#[path = "pickers_test.rs"]
mod tests;
