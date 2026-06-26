//! Text-selection state for the content pane.
//!
//! `TextSelection` models a character-range selection as an `anchor` and an
//! `active` `(line, column)` endpoint that may extend in either direction;
//! `normalized()` returns them in start-to-end order for rendering and copying.
//! The hit-testing, extension, and clipboard copying live in the app's
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

#[cfg(test)]
#[path = "selection_test.rs"]
mod tests;
