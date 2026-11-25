//! Picker: filter bar.

use crate::list_picker::ListPicker;

/// State for the log lines filter bar.
pub struct FilterBarState {
    pub query: String,
}

impl Default for FilterBarState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterBarState {
    pub fn new() -> Self {
        FilterBarState {
            query: String::new(),
        }
    }
}

impl ListPicker for FilterBarState {
    fn query_push(&mut self, c: char) {
        self.query.push(c);
    }
    fn query_pop(&mut self) {
        self.query.pop();
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        0
    }
    fn selected(&self) -> usize {
        0
    }
    fn set_selected(&mut self, _i: usize) {}
}
