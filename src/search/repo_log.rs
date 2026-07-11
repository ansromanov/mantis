//! Fuzzy-filterable list of repository-wide git commits with paged loading.
//!
//! `RepoLogState` holds the commit list for the entire repository's git log
//! and supports fuzzy filtering by subject/author/hash via `SkimMatcherV2`.
//! Commits are loaded in pages of `PAGE_SIZE`; more are fetched when the
//! user scrolls past the loaded set.

use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// Number of commits fetched per page from `git log`.
const PAGE_SIZE: usize = 200;

/// Fuzzy-filterable list of repository-wide commits with paged loading.
pub struct RepoLogState {
    pub repo_dir: PathBuf,
    pub commits: Vec<crate::git::Commit>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    matcher: SkimMatcherV2,
    total_loaded: usize,
    has_more: bool,
}

impl RepoLogState {
    pub fn new(repo_dir: PathBuf) -> Self {
        let commits = crate::git::repo_log(&repo_dir, 0, PAGE_SIZE);
        let has_more = commits.len() == PAGE_SIZE;
        let total_loaded = commits.len();
        let filtered = (0..commits.len()).collect();
        RepoLogState {
            repo_dir,
            commits,
            query: String::new(),
            filtered,
            selected: 0,
            matcher: SkimMatcherV2::default(),
            total_loaded,
            has_more,
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

    /// Loads the next page of commits when the user scrolls past the end
    /// of the currently loaded set. Returns `true` if new commits were loaded.
    pub fn load_more(&mut self) -> bool {
        if !self.has_more {
            return false;
        }
        let more = crate::git::repo_log(&self.repo_dir, self.total_loaded, PAGE_SIZE);
        if more.is_empty() {
            self.has_more = false;
            return false;
        }
        self.has_more = more.len() == PAGE_SIZE;
        self.total_loaded += more.len();
        self.commits.extend(more);
        self.refilter();
        true
    }

    fn refilter(&mut self) {
        self.selected = 0;
        self.filtered = super::fuzzy_refilter(
            &self.commits,
            &self.matcher,
            &self.query,
            |c| {
                std::borrow::Cow::Owned(format!(
                    "{} {} {} {}",
                    c.short, c.date, c.author, c.subject
                ))
            },
            false,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    }
}

impl ListPicker for RepoLogState {
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
#[path = "repo_log_test.rs"]
mod tests;
