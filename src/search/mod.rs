//! Fuzzy file and content search, plus the shared fuzzy-picker overlays.
//!
//! Provides the search engine and the state behind several look-alike overlays.
//! `SearchState` drives the main picker, which fuzzy-matches either file paths
//! (`SearchMode::Files`) or file contents (`SearchMode::Content`) using
//! `SkimMatcherV2`, debouncing the expensive content scans. `ContentMatch`
//! carries a hit's path, line number, and surrounding context. The same
//! query/filtered-list/selected-index shape backs `HistoryState`, `ThemePicker`,
//! `CommandPalette`, `RecentFilesState`, `PluginPicker`, the in-file search (`InFileSearch`),
//! and the go-to-line dialog (`GotoLineState`), all defined here. The shared
//! `fuzzy_refilter` helper scores and sorts any typed item list by descending fuzzy
//! score; binary files are skipped via `is_binary_bytes`.

mod history;
mod pickers;

pub use history::HistoryState;
// All picker types are re-exported for `crate::search` consumers.
// `InFileMatch` is separated to avoid an unused-import warning in the binary
// target (it is used only by test code and as a field type of `InFileSearch`).
#[allow(unused_imports)]
pub use pickers::{
    GotoLineState, InFileMatch, InFileSearch, PluginPicker, RecentFilesState, ThemePicker,
    TreeFilter,
};

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

use crate::file::is_binary_bytes;
use crate::tree::collect_all_files;

pub(crate) fn fuzzy_refilter<T>(
    items: &[T],
    matcher: &SkimMatcherV2,
    query: &str,
    haystack: impl for<'a> Fn(&'a T) -> std::borrow::Cow<'a, str>,
) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let mut scored: Vec<(usize, i64)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| {
            matcher
                .fuzzy_match(haystack(item).as_ref(), query)
                .map(|s| (i, s))
        })
        .collect();
    scored.sort_by_key(|(_, s)| std::cmp::Reverse(*s));
    scored.into_iter().map(|(i, _)| i).collect()
}

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
    /// When `true`, the search index is scoped to a subset of files (e.g. git
    /// mode's changed files). Displayed as "(changed files)" in the overlay title.
    pub scoped: bool,
    /// Interpret the content query as a regular expression (Ctrl+R).
    pub regex: bool,
    /// Match case-sensitively instead of the default smart-insensitive (Ctrl+A).
    pub case_sensitive: bool,
    /// Match whole words only (Ctrl+W).
    pub whole_word: bool,
}

/// Compiles the regex used when the `regex` or `whole_word` search options are
/// on, shared by the content search and the in-file search. A non-regex
/// whole-word query is escaped first. Returns `None` while the pattern is
/// invalid (e.g. a regex mid-typing), which callers treat as "no matches".
pub(crate) fn build_search_regex(
    query: &str,
    is_regex: bool,
    whole_word: bool,
    case_sensitive: bool,
) -> Option<regex::Regex> {
    let pattern = if whole_word {
        let inner = if is_regex {
            query.to_string()
        } else {
            regex::escape(query)
        };
        format!(r"\b(?:{inner})\b")
    } else {
        query.to_string()
    };
    regex::RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
        .ok()
}

fn filter_changed_paths(paths: &HashSet<PathBuf>, root: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = paths
        .iter()
        .filter(|p| p.starts_with(root))
        .cloned()
        .collect();
    files.sort();
    files
}

impl SearchState {
    pub fn new(
        root: &Path,
        show_hidden: bool,
        ignore_gitignore: bool,
        context_lines: usize,
        changed_only: Option<&HashSet<PathBuf>>,
    ) -> Self {
        let all_files = match changed_only {
            Some(paths) => filter_changed_paths(paths, root),
            None => collect_all_files(root, show_hidden, ignore_gitignore),
        };
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
            scoped: changed_only.is_some(),
            regex: false,
            case_sensitive: false,
            whole_word: false,
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

    pub fn reload_files(
        &mut self,
        root: &Path,
        show_hidden: bool,
        ignore_gitignore: bool,
        changed_only: Option<&HashSet<PathBuf>>,
    ) {
        self.all_files = match changed_only {
            Some(paths) => filter_changed_paths(paths, root),
            None => collect_all_files(root, show_hidden, ignore_gitignore),
        };
        self.scoped = changed_only.is_some();
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
        let ctx = self.context_lines;
        let re = if self.regex || self.whole_word {
            let Some(re) = build_search_regex(
                &self.query,
                self.regex,
                self.whole_word,
                self.case_sensitive,
            ) else {
                return;
            };
            Some(re)
        } else {
            None
        };
        let q = if self.case_sensitive {
            self.query.clone()
        } else {
            self.query.to_lowercase()
        };
        for path in &self.all_files {
            let Some(lines) = self.content_cache.get(path) else {
                continue;
            };
            for (i, (orig, lower)) in lines.iter().enumerate() {
                let matched = match &re {
                    Some(re) => re.is_match(orig),
                    None if self.case_sensitive => orig.contains(&q),
                    None => lower.contains(&q),
                };
                if matched {
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

pub use crate::command_palette::CommandPalette;

use crate::list_picker::ListPicker;

impl ListPicker for SearchState {
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
#[path = "../search_test.rs"]
mod tests;
