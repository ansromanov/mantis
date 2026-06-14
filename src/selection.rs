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
mod tests {
    use super::*;

    #[test]
    fn normalized_returns_ordered_pair() {
        let sel = TextSelection {
            anchor: (5, 3),
            active: (2, 7),
        };
        let (start, end) = sel.normalized();
        assert_eq!(start, (2, 7));
        assert_eq!(end, (5, 3));
    }

    #[test]
    fn normalized_returns_same_when_already_ordered() {
        let sel = TextSelection {
            anchor: (2, 7),
            active: (5, 3),
        };
        let (start, end) = sel.normalized();
        assert_eq!(start, (2, 7));
        assert_eq!(end, (5, 3));
    }

    #[test]
    fn normalized_equal_positions() {
        let sel = TextSelection {
            anchor: (3, 5),
            active: (3, 5),
        };
        let (start, end) = sel.normalized();
        assert_eq!(start, (3, 5));
        assert_eq!(end, (3, 5));
    }

    #[test]
    fn is_empty_returns_true_when_equal() {
        let sel = TextSelection {
            anchor: (1, 1),
            active: (1, 1),
        };
        assert!(sel.is_empty());
    }

    #[test]
    fn is_empty_returns_false_when_different() {
        let sel = TextSelection {
            anchor: (1, 1),
            active: (1, 2),
        };
        assert!(!sel.is_empty());
    }

    #[test]
    fn visual_line_new_collapses_to_single_line() {
        let v = VisualLine::new(7);
        assert_eq!(v.range(), (7, 7));
    }

    #[test]
    fn visual_line_range_orders_downward_selection() {
        let v = VisualLine {
            anchor: 10,
            cursor: 4,
        };
        assert_eq!(v.range(), (4, 10));
    }

    #[test]
    fn visual_line_range_orders_upward_selection() {
        let v = VisualLine {
            anchor: 4,
            cursor: 10,
        };
        assert_eq!(v.range(), (4, 10));
    }
}
