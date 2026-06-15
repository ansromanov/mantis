//! Hunk-to-hunk navigation within a diff for `App`.
//!
//! When the content pane shows a diff, these helpers let the user jump between
//! `@@` hunk headers instead of scrolling line by line. `diff_hunk_rows`
//! collects the display-row index of every hunk header in the coordinate space
//! of the active layout (unified or side-by-side), and the `diff_next_hunk` /
//! `diff_prev_hunk` methods move the scroll position to the nearest header above
//! or below the current viewport. They are no-ops when the current content is
//! not a diff, so they can be wired to keybindings unconditionally.

use super::App;

impl App {
    /// Returns the display-row indices of hunk headers (`@@`) in the current
    /// diff, in the coordinate space matching the active layout.
    fn diff_hunk_rows(&self) -> Vec<usize> {
        if self.diff_sbs_active() {
            self.diff_rows
                .iter()
                .enumerate()
                .filter(|(_, r)| matches!(r, crate::diff::DiffRow::Header(_)))
                .map(|(i, _)| i)
                .collect()
        } else {
            self.content
                .iter()
                .enumerate()
                .filter(|(_, l)| l.starts_with("@@"))
                .map(|(i, _)| i)
                .collect()
        }
    }

    /// Scrolls to the next hunk header below the current scroll position.
    pub(crate) fn diff_next_hunk(&mut self) {
        let cur = self.content_scroll;
        if let Some(&next) = self.diff_hunk_rows().iter().find(|&&i| i > cur) {
            self.content_scroll = next.min(self.content_scroll_max());
            self.mark_content_scrolled();
        }
    }

    /// Scrolls to the previous hunk header above the current scroll position.
    pub(crate) fn diff_prev_hunk(&mut self) {
        let cur = self.content_scroll;
        if let Some(&prev) = self.diff_hunk_rows().iter().rev().find(|&&i| i < cur) {
            self.content_scroll = prev;
            self.mark_content_scrolled();
        }
    }
}
