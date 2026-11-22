//! Picker: theme.

use fuzzy_matcher::skim::SkimMatcherV2;

use crate::list_picker::ListPicker;

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
    pub(super) matcher: SkimMatcherV2,
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
        self.filtered = crate::search::fuzzy_refilter(
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
