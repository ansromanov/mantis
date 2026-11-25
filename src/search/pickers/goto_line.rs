//! Picker: goto line.

use crate::list_picker::ListPicker;

/// State for the go-to-line dialog.
///
/// Opened by the `goto_line` keybinding (default `:`). The user types a line
/// number and presses Enter to jump (1-indexed, clamped to valid range), or Esc
/// to cancel. Supports relative jumps: `+N` jumps forward N lines, `-N` jumps
/// backward N lines.
pub struct GotoLineState {
    pub query: String,
}

impl Default for GotoLineState {
    fn default() -> Self {
        Self::new()
    }
}

impl GotoLineState {
    pub fn new() -> Self {
        GotoLineState {
            query: String::new(),
        }
    }

    pub fn push(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn pop(&mut self) {
        self.query.pop();
    }
}

impl ListPicker for GotoLineState {
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
        0
    }
    fn selected(&self) -> usize {
        0
    }
    fn set_selected(&mut self, _i: usize) {}
}
