//! Picker: symbol.

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use crate::code_intel::Symbol;
use crate::list_picker::ListPicker;

/// Fuzzy-filterable symbols for the command-palette route and outline overlay.
pub struct SymbolPicker {
    /// Source symbols supplied by the active language provider.
    pub symbols: Vec<Symbol>,
    /// Current fuzzy query.
    pub query: String,
    /// Indices into `symbols`, ordered by descending fuzzy score.
    pub filtered: Vec<usize>,
    /// Selected index into `filtered`.
    pub selected: usize,
    pub(super) matcher: SkimMatcherV2,
}

impl SymbolPicker {
    /// Creates a picker and initially selects the symbol containing `line`.
    pub fn new(symbols: Vec<Symbol>, line: Option<usize>) -> Self {
        let selected_source = line.and_then(|line| {
            crate::code_intel::enclosing_symbol(&symbols, line)
                .and_then(|current| symbols.iter().position(|symbol| symbol == current))
        });
        let mut picker = Self {
            filtered: (0..symbols.len()).collect(),
            symbols,
            query: String::new(),
            selected: 0,
            matcher: SkimMatcherV2::default(),
        };
        if let Some(source_index) = selected_source {
            picker.selected = picker
                .filtered
                .iter()
                .position(|index| *index == source_index)
                .unwrap_or(0);
        }
        picker
    }

    /// Returns the symbol at the selected filtered index.
    pub fn selected_symbol(&self) -> Option<&Symbol> {
        self.filtered
            .get(self.selected)
            .and_then(|index| self.symbols.get(*index))
    }

    fn refresh(&mut self) {
        if self.query.is_empty() {
            self.filtered = (0..self.symbols.len()).collect();
        } else {
            let mut scored: Vec<(usize, i64)> = self
                .symbols
                .iter()
                .enumerate()
                .filter_map(|(index, symbol)| {
                    let text = format!(
                        "{} {} {}",
                        symbol.kind,
                        symbol.name,
                        symbol.parent.as_deref().unwrap_or("")
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

impl ListPicker for SymbolPicker {
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
