//! Picker: diagnostic.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::code_intel::Diagnostic;
use crate::list_picker::ListPicker;

/// Fuzzy-filterable diagnostics for the current file and diagnostics palette route.
pub struct DiagnosticPicker {
    /// Diagnostics returned by the active language provider.
    pub diagnostics: Vec<Diagnostic>,
    /// Current fuzzy query.
    pub query: String,
    /// Indices into `diagnostics`, ordered by descending fuzzy score.
    pub filtered: Vec<usize>,
    /// Selected index into `filtered`.
    pub selected: usize,
    pub(super) matcher: SkimMatcherV2,
}

impl DiagnosticPicker {
    /// Creates a picker over one file's diagnostics.
    pub fn new(diagnostics: Vec<Diagnostic>) -> Self {
        let mut picker = Self {
            filtered: (0..diagnostics.len()).collect(),
            diagnostics,
            query: String::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
        };
        picker.refresh();
        picker
    }

    /// Returns the currently selected diagnostic.
    pub fn selected_diagnostic(&self) -> Option<&Diagnostic> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.diagnostics.get(*index))
    }

    /// Replaces results after a file reload while preserving the search query.
    pub fn set_diagnostics(&mut self, diagnostics: Vec<Diagnostic>) {
        self.diagnostics = diagnostics;
        self.refresh();
    }

    fn refresh(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.diagnostics.len()).collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .diagnostics
                .iter()
                .enumerate()
                .filter_map(|(index, diagnostic)| {
                    let text = format!(
                        "{} {} {} {}:{}",
                        diagnostic.severity.label(),
                        diagnostic.source,
                        diagnostic.message,
                        diagnostic.line + 1,
                        diagnostic.column + 1
                    );
                    self.matcher
                        .fuzzy_match(&text, &self.query)
                        .map(|score| (index, score))
                })
                .collect();
            scored.sort_by_key(|(_, score)| std::cmp::Reverse(*score));
            self.filtered = scored.into_iter().map(|(index, _)| index).collect();
        }
        self.selected = self.selected.min(self.filtered.len().saturating_sub(1));
    }

    pub(crate) fn push(&mut self, c: char) {
        self.query.push(c);
        self.refresh();
    }

    pub(crate) fn pop(&mut self) {
        self.query.pop();
        self.refresh();
    }
}

impl ListPicker for DiagnosticPicker {
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
        self.filtered.len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, index: usize) {
        if index < self.filtered.len() {
            self.selected = index;
        }
    }
}
