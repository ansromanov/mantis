//! Picker: tab.

use std::path::PathBuf;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::list_picker::ListPicker;

/// One open project represented in the workspace tab picker.
#[derive(Debug, Clone)]
pub struct TabPickerItem {
    /// The tab's original index in `Tabs::apps`.
    pub index: usize,
    /// Root path shown to distinguish similarly named projects.
    pub root: PathBuf,
    /// Branch or repository state, when the root is a git repository.
    pub branch: Option<String>,
    /// Number of changed files reported by the app's current git snapshot.
    pub changed: usize,
}

impl TabPickerItem {
    /// Formats the root, branch, and changed-file count for the picker row.
    pub fn display(&self) -> String {
        let mut row = self.root.display().to_string();
        if let Some(branch) = &self.branch {
            row.push_str(" · ");
            row.push_str(branch);
            if self.changed > 0 {
                row.push_str(&format!(" ●{}", self.changed));
            }
        }
        row
    }
}

/// Fuzzy-filterable overview of the tabs in the current workspace.
pub struct TabPicker {
    /// Tabs represented by this picker, in their original order.
    pub items: Vec<TabPickerItem>,
    /// Current fuzzy-search query.
    pub query: String,
    /// Indices into `items`, ordered by fuzzy relevance.
    pub filtered: Vec<usize>,
    /// Selected position in `filtered`.
    pub selected: usize,
    pub(super) matcher: SkimMatcherV2,
}

impl TabPicker {
    /// Builds a picker snapshot from the currently open apps.
    pub fn new(apps: &[crate::app::App]) -> Self {
        let items = apps
            .iter()
            .enumerate()
            .map(|(index, app)| TabPickerItem {
                index,
                root: app.root.clone(),
                branch: app.git_info.as_ref().map(|info| info.head.display()),
                changed: app.git_info.as_ref().map_or(0, |info| info.total_changed),
            })
            .collect();
        let mut picker = Self {
            items,
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
        };
        picker.refresh();
        picker
    }

    /// Recomputes results and keeps the selection within the filtered list.
    pub fn refresh(&mut self) {
        self.filtered = if self.query.is_empty() {
            (0..self.items.len()).collect()
        } else {
            let mut scored = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| {
                    self.matcher
                        .fuzzy_match(&item.display(), &self.query)
                        .map(|score| (index, score))
                })
                .collect::<Vec<_>>();
            scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
            scored.into_iter().map(|(index, _)| index).collect()
        };
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    /// Returns the original tab index currently selected, if any.
    pub fn selected_tab(&self) -> Option<usize> {
        self.filtered
            .get(self.selected)
            .and_then(|&item| self.items.get(item))
            .map(|item| item.index)
    }
}

impl ListPicker for TabPicker {
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

    fn set_selected(&mut self, index: usize) {
        self.selected = index.min(self.filtered.len().saturating_sub(1));
    }
}
