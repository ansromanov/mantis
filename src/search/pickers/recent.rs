//! Picker: recent.

use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// Fuzzy-filterable list of recently opened files, most-recent-first.
pub struct RecentFilesState {
    pub paths: Vec<PathBuf>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub(super) matcher: SkimMatcherV2,
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
        self.filtered = crate::search::fuzzy_refilter(
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
