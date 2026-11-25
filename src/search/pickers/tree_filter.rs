//! Picker: tree filter.

use std::collections::HashSet;
use std::path::PathBuf;

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
