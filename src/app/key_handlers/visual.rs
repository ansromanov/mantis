//! Visual-line selection mode for the content pane.
//!
//! Visual-line mode lets the user select whole content lines with the keyboard
//! (anchored at one line, extended with motion) and then act on the range - most
//! notably opening a selection-scoped git-blame panel. `enter_visual_line`
//! starts the mode at the first visible line, and `handle_visual_line_key`
//! processes movement, range extension, blame-panel toggling, and Esc to exit.
//! The selection is stored in `App::visual_line`; this module only manipulates
//! that state and the blame-panel flag, deferring all rendering to the UI
//! layer.

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::pressed;
use crate::selection::VisualLine;

use super::super::App;

impl App {
    /// Enters visual-line mode with the cursor anchored at the first visible
    /// content line. A no-op when no file is open.
    pub(crate) fn enter_visual_line(&mut self) {
        if self.current_file.is_none() || self.line_count() == 0 {
            return;
        }
        let start = self
            .content_scroll
            .min(self.display_line_count().saturating_sub(1));
        self.visual_line = Some(VisualLine::new(start));
        self.blame_panel = false;
    }

    /// Leaves visual-line mode and dismisses any scoped blame panel.
    pub(crate) fn exit_visual_line(&mut self) {
        self.visual_line = None;
        self.blame_panel = false;
    }

    /// Handles keys while visual-line mode is active: navigation extends the
    /// selection, the blame key toggles the scoped panel, and Esc (or the
    /// toggle key) exits.
    pub(super) fn handle_visual_line_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if pressed(&k.quit, &key) {
            self.should_quit = true;
            return;
        }
        if key.code == KeyCode::Esc || pressed(&k.visual_line_toggle, &key) {
            self.exit_visual_line();
            return;
        }
        if pressed(&k.visual_line_blame, &key) {
            self.blame_panel = !self.blame_panel;
            return;
        }

        let max_line = self.display_line_count().saturating_sub(1);
        let moved = if pressed(&k.nav_up, &key) {
            self.move_visual_cursor(|c| c.saturating_sub(1), max_line)
        } else if pressed(&k.nav_down, &key) {
            self.move_visual_cursor(|c| c + 1, max_line)
        } else if pressed(&k.content_page_up, &key) {
            self.move_visual_cursor(|c| c.saturating_sub(20), max_line)
        } else if pressed(&k.content_page_down, &key) {
            self.move_visual_cursor(|c| c + 20, max_line)
        } else if pressed(&k.content_top, &key) {
            self.move_visual_cursor(|_| 0, max_line)
        } else if pressed(&k.content_bottom, &key) {
            self.move_visual_cursor(|_| max_line, max_line)
        } else {
            false
        };

        if moved {
            self.scroll_visual_cursor_into_view();
            self.mark_content_scrolled();
        }
    }

    /// Applies `f` to the visual-line cursor, clamping the result to `max_line`.
    /// Returns `true` if a selection is active (so the caller can react).
    fn move_visual_cursor(&mut self, f: impl FnOnce(usize) -> usize, max_line: usize) -> bool {
        if let Some(v) = &mut self.visual_line {
            v.cursor = f(v.cursor).min(max_line);
            true
        } else {
            false
        }
    }

    /// Nudges `content_scroll` so the visual-line cursor stays within the
    /// viewport after a move.
    fn scroll_visual_cursor_into_view(&mut self) {
        let view_height = (self.content_area.height as usize).max(1);
        let Some(v) = &self.visual_line else {
            return;
        };
        let cursor = v.cursor;
        if cursor < self.content_scroll {
            self.content_scroll = cursor;
        } else if cursor >= self.content_scroll + view_height {
            self.content_scroll = cursor.saturating_sub(view_height).saturating_add(1);
        }
    }
}
