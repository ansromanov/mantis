//! Picker: revision.

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

/// A single selectable revision in the revision picker.
#[derive(Debug, Clone)]
pub struct RevisionItem {
    /// The revision specifier to pass to `git diff` (short hash, branch name,
    /// tag name, or shortcut like `HEAD` / `HEAD~1`).
    pub rev: String,
    /// Human-readable label shown in the picker list.
    pub display: String,
}

/// Tab categories for the revision picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionTab {
    Commits,
    Tags,
    Branches,
}

impl RevisionTab {
    /// The display label for this tab.
    pub fn label(self) -> &'static str {
        match self {
            RevisionTab::Commits => "Commits",
            RevisionTab::Tags => "Tags",
            RevisionTab::Branches => "Branches",
        }
    }

    /// All tabs in display order.
    pub const ALL: [RevisionTab; 3] = [
        RevisionTab::Commits,
        RevisionTab::Tags,
        RevisionTab::Branches,
    ];
}

/// State for the revision picker overlay.
///
/// Presents selectable revisions in tabbed categories: Commits (default),
/// Tags, and Branches. Supports fuzzy filtering within the active tab and
/// retains free-form entry — if the filtered list is empty but the user has
/// typed something, pressing Enter uses the typed text as a raw revspec.
/// Left/Right arrows switch tabs when the query is empty.
pub struct RevisionPicker {
    pub items: Vec<RevisionItem>,
    pub query: String,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub(crate) matcher: SkimMatcherV2,
    /// The currently active tab.
    pub tab: RevisionTab,
    /// Items separated by category.
    pub(crate) shortcuts: Vec<RevisionItem>,
    pub(crate) commits: Vec<RevisionItem>,
    pub(crate) tags: Vec<RevisionItem>,
    pub(crate) branches: Vec<RevisionItem>,
}

impl RevisionPicker {
    /// Builds the revision list by shelling out to git. When git is unavailable
    /// or the directory is not a repo, items is empty (the picker still allows
    /// free-form entry).
    pub fn new(repo_dir: &std::path::Path) -> Self {
        Self::new_for_tab(repo_dir, RevisionTab::Commits)
    }

    /// Builds the revision picker with `tab` selected initially.
    pub fn new_for_tab(repo_dir: &std::path::Path, tab: RevisionTab) -> Self {
        let mut shortcuts = vec![
            RevisionItem {
                rev: "HEAD".into(),
                display: "HEAD (current)".into(),
            },
            RevisionItem {
                rev: "HEAD~1".into(),
                display: "HEAD~1 (parent)".into(),
            },
            RevisionItem {
                rev: "HEAD~2".into(),
                display: "HEAD~2 (grandparent)".into(),
            },
        ];
        shortcuts.retain(|item| crate::git::verify_revision(repo_dir, &item.rev).is_ok());

        let mut commits = Vec::new();
        for commit in crate::git::recent_commits(repo_dir, 50) {
            commits.push(RevisionItem {
                rev: commit.hash,
                display: format!("{} {}", commit.short, commit.subject),
            });
        }

        let mut tags = Vec::new();
        for tag in crate::git::tags(repo_dir) {
            tags.push(RevisionItem {
                rev: tag.clone(),
                display: tag,
            });
        }

        let mut branches = Vec::new();
        for branch in crate::git::branches(repo_dir) {
            branches.push(RevisionItem {
                rev: branch.clone(),
                display: branch,
            });
        }

        let mut picker = RevisionPicker {
            items: Vec::new(),
            query: String::new(),
            filtered: Vec::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
            tab,
            shortcuts,
            commits,
            tags,
            branches,
        };
        picker.rebuild_items();
        picker
    }

    /// Rebuilds `items` and `filtered` from the current tab's category lists
    /// plus shortcuts, then re-applies the active query filter.
    pub(crate) fn rebuild_items(&mut self) {
        self.items = self.shortcuts.clone();
        match self.tab {
            RevisionTab::Commits => self.items.extend(self.commits.clone()),
            RevisionTab::Tags => self.items.extend(self.tags.clone()),
            RevisionTab::Branches => self.items.extend(self.branches.clone()),
        }
        self.refilter();
    }

    /// Switches to the next tab (wrapping) and rebuilds the items list.
    pub fn next_tab(&mut self) {
        let idx = RevisionTab::ALL
            .iter()
            .position(|&t| t == self.tab)
            .unwrap_or(0);
        self.tab = RevisionTab::ALL[(idx + 1) % RevisionTab::ALL.len()];
        self.rebuild_items();
    }

    /// Switches to the previous tab (wrapping) and rebuilds the items list.
    pub fn prev_tab(&mut self) {
        let idx = RevisionTab::ALL
            .iter()
            .position(|&t| t == self.tab)
            .unwrap_or(0);
        self.tab = RevisionTab::ALL[(idx + RevisionTab::ALL.len() - 1) % RevisionTab::ALL.len()];
        self.rebuild_items();
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

    pub fn selected_rev(&self) -> Option<&str> {
        self.filtered
            .get(self.selected)
            .and_then(|&i| self.items.get(i))
            .map(|item| item.rev.as_str())
    }

    pub(crate) fn refilter(&mut self) {
        self.selected = 0;
        self.filtered = crate::search::fuzzy_refilter(
            &self.items,
            &self.matcher,
            &self.query,
            |item| std::borrow::Cow::Borrowed(item.display.as_str()),
            false,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    }
}

impl ListPicker for RevisionPicker {
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
impl RevisionPicker {
    /// Creates a picker from a flat list of items for testing purposes.
    /// Items are treated as the commits category; shortcuts/tags/branches
    /// are left empty.
    pub(crate) fn for_test(items: Vec<RevisionItem>) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        RevisionPicker {
            items,
            query: String::new(),
            filtered,
            selected: 0,
            matcher: SkimMatcherV2::default(),
            tab: RevisionTab::Commits,
            shortcuts: Vec::new(),
            commits: Vec::new(),
            tags: Vec::new(),
            branches: Vec::new(),
        }
    }
}
