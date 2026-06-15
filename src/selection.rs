//! Text-selection state for the content pane.
//!
//! `TextSelection` models a character-range selection as an `anchor` and an
//! `active` `(line, column)` endpoint that may extend in either direction;
//! `normalized()` returns them in start-to-end order for rendering and copying.
//! `VisualLine` is the whole-line counterpart used by visual-line mode, tracking
//! the anchored and current line so a range of full lines can be selected and,
//! for example, fed to the scoped git-blame panel. Both are plain state holders:
//! the hit-testing, extension, and clipboard copying live in the app's
//! mouse/key handlers and the UI rendering layer.

/// A text selection spanning from `anchor` to `active`, each being a
/// `(line, column)` pair. The selection may extend in any direction; use
/// `normalized()` to obtain a canonical start-end ordering.
pub struct TextSelection {
    pub anchor: (usize, usize),
    pub active: (usize, usize),
}

impl TextSelection {
    /// Returns `(anchor, active)` ordered so the first tuple is <= the second.
    pub fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.active {
            (self.anchor, self.active)
        } else {
            (self.active, self.anchor)
        }
    }

    /// Returns `true` when anchor and active are at the same position.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.active
    }
}

/// A vim-style visual-line selection in the content panel. Both `anchor` and
/// `cursor` are **display-line** indices (the same coordinate space as
/// `content_scroll`); `cursor` is the line that moves as the user navigates.
#[derive(Clone, Copy)]
pub struct VisualLine {
    pub anchor: usize,
    pub cursor: usize,
}

impl VisualLine {
    /// Starts a selection anchored at `line`, with the cursor on the same line.
    pub fn new(line: usize) -> Self {
        VisualLine {
            anchor: line,
            cursor: line,
        }
    }

    /// Returns the selected line range as an inclusive `(start, end)` pair
    /// ordered so `start <= end`.
    pub fn range(&self) -> (usize, usize) {
        if self.anchor <= self.cursor {
            (self.anchor, self.cursor)
        } else {
            (self.cursor, self.anchor)
        }
    }
}

#[cfg(test)]
#[path = "selection_test.rs"]
mod tests;
