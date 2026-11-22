//! Picker: worktree.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

const WORKTREE_CACHE_TTL: Duration = Duration::from_secs(5);

struct WorktreeCacheEntry {
    updated: Instant,
    items: Vec<crate::git::WorktreeItem>,
}

static WORKTREE_CACHE: OnceLock<Mutex<HashMap<PathBuf, WorktreeCacheEntry>>> = OnceLock::new();

fn worktree_cache() -> &'static Mutex<HashMap<PathBuf, WorktreeCacheEntry>> {
    WORKTREE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Fuzzy-filterable worktree overview shown by the worktree switcher.
pub struct WorktreePicker {
    pub items: Vec<crate::git::WorktreeItem>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    /// Makes Enter open the selection in a tab. Ctrl+Enter always does so.
    pub open_in_new_tab: bool,
    pub(super) matcher: SkimMatcherV2,
    pub(super) changed_rx: Option<Receiver<Vec<(PathBuf, usize)>>>,
    cache_key: PathBuf,
}

impl WorktreePicker {
    #[cfg(test)]
    pub fn for_test(items: Vec<crate::git::WorktreeItem>) -> Self {
        let mut picker = Self {
            items,
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
            open_in_new_tab: false,
            matcher: SkimMatcherV2::default(),
            changed_rx: None,
            cache_key: PathBuf::new(),
        };
        picker.refresh();
        picker
    }

    pub fn new(root: &std::path::Path) -> Self {
        let cache_key = root.to_path_buf();
        let cached = worktree_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&cache_key)
            .filter(|entry| entry.updated.elapsed() < WORKTREE_CACHE_TTL)
            .map(|entry| entry.items.clone());
        let needs_scan = cached.is_none();
        let items = cached.unwrap_or_else(|| {
            crate::git::worktree_list(root)
                .into_iter()
                .map(|worktree| crate::git::WorktreeItem {
                    worktree,
                    changed: 0,
                })
                .collect()
        });
        let paths = items
            .iter()
            .map(|item| item.worktree.path.clone())
            .collect::<Vec<_>>();
        let changed_rx = if needs_scan {
            let (changed_tx, changed_rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("mantis-worktree-info".into())
                .spawn(move || {
                    let updates = paths
                        .into_iter()
                        .map(|path| {
                            let changed =
                                crate::git::repo_info(&path).map_or(0, |info| info.total_changed);
                            (path, changed)
                        })
                        .collect();
                    let _ = changed_tx.send(updates);
                })
                .ok();
            Some(changed_rx)
        } else {
            None
        };
        let mut picker = Self {
            items,
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
            open_in_new_tab: false,
            matcher: SkimMatcherV2::default(),
            changed_rx,
            cache_key,
        };
        picker.refresh();
        picker
    }

    /// Builds the picker with Enter set to open the selection in a tab.
    pub fn new_tab(root: &std::path::Path) -> Self {
        let mut picker = Self::new(root);
        picker.open_in_new_tab = true;
        picker
    }

    /// Applies any completed background changed-file count scan.
    pub(crate) fn poll_changed_counts(&mut self) {
        let Some(rx) = &self.changed_rx else {
            return;
        };
        let updates = match rx.try_recv() {
            Ok(updates) => updates,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.changed_rx = None;
                return;
            }
        };
        self.changed_rx = None;
        for (path, changed) in updates {
            if let Some(item) = self
                .items
                .iter_mut()
                .find(|item| item.worktree.path == path)
            {
                item.changed = changed;
            }
        }
        worktree_cache()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                self.cache_key.clone(),
                WorktreeCacheEntry {
                    updated: Instant::now(),
                    items: self.items.clone(),
                },
            );
        self.refresh();
    }

    fn refresh(&mut self) {
        self.filtered = crate::search::fuzzy_refilter(
            &self.items,
            &self.matcher,
            &self.query,
            |item| std::borrow::Cow::Owned(item.display()),
            false,
        )
        .into_iter()
        .map(|(index, _)| index)
        .collect();
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub fn selected_path(&self) -> Option<std::path::PathBuf> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
            .map(|i| i.worktree.path.clone())
    }
}

impl ListPicker for WorktreePicker {
    fn query_push(&mut self, c: char) {
        self.query.push(c);
        self.refresh();
    }
    fn query_pop(&mut self) {
        self.query.pop();
        self.refresh();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.filtered.len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i.min(self.filtered.len().saturating_sub(1));
    }
}
