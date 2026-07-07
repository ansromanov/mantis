//! Fuzzy-filterable list of git commits that touched a single file.
//!
//! `HistoryState` holds the commit list for a file's git log and supports
//! fuzzy filtering by commit message/date/hash via `SkimMatcherV2`.

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// Fuzzy-filterable list of the commits that touched a single file.
pub struct HistoryState {
    pub file: std::path::PathBuf,
    pub commits: Vec<crate::git::Commit>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    matcher: SkimMatcherV2,
}

impl HistoryState {
    pub fn new(file: std::path::PathBuf, commits: Vec<crate::git::Commit>) -> Self {
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
        self.filtered = super::fuzzy_refilter(
            &self.commits,
            &self.matcher,
            &self.query,
            |c| std::borrow::Cow::Owned(format!("{} {} {}", c.short, c.date, c.subject)),
            false,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    }
}

impl ListPicker for HistoryState {
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

#[cfg(test)]
#[path = "history_test.rs"]
mod tests;
