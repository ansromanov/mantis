//! Key handling for the fuzzy-picker overlays.
//!
//! Each overlay handler is a thin wrapper around the shared
//! [`handle_list_picker_key`] dispatcher: handle extra keys first, then
//! fall through to the dispatcher and map `Activate`/`Close` to the
//! overlay-specific action.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::list_picker::{handle_list_picker_key, OverlayKey};

use super::super::App;

impl App {
    /// Handles keyboard input while the search overlay is open.
    /// Extra keys: Tab toggles file/content mode.
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) {
        // Extra keys first
        if let KeyCode::Tab = key.code {
            if let Some(s) = &mut self.search {
                s.toggle_mode();
            }
            return;
        }
        let Some(ref mut s) = self.search else { return };
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate => self.activate_search_selection(),
            OverlayKey::Close => {
                self.last_search_query = s.query.clone();
                self.search = None;
            }
            _ => {}
        }
    }

    /// Handles keyboard input while in-file search is active.
    /// Extra keys: n/N/Tab/BackTab/Ctrl-p navigate matches.
    pub(super) fn handle_in_file_search_key(&mut self, key: KeyEvent) {
        // Extra keys first
        match key.code {
            KeyCode::Char('n') => {
                self.in_file_search_next();
                return;
            }
            KeyCode::Char('N') | KeyCode::Char('P') => {
                self.in_file_search_prev();
                return;
            }
            KeyCode::Tab => {
                self.in_file_search_next();
                return;
            }
            KeyCode::BackTab => {
                self.in_file_search_prev();
                return;
            }
            KeyCode::Char('p') if key.modifiers.intersects(KeyModifiers::CONTROL) => {
                self.in_file_search_prev();
                return;
            }
            _ => {}
        }
        let Some(ref mut s) = self.in_file_search else {
            return;
        };
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate | OverlayKey::Close => {
                self.in_file_search = None;
            }
            OverlayKey::Handled => {
                self.refresh_in_file_search();
                self.scroll_in_file_search_to_current();
            }
            _ => {}
        }
    }

    pub(crate) fn in_file_search_next(&mut self) {
        if let Some(s) = &mut self.in_file_search {
            if !s.matches.is_empty() {
                s.current = (s.current + 1) % s.matches.len();
            }
        }
        self.scroll_in_file_search_to_current();
    }

    pub(crate) fn in_file_search_prev(&mut self) {
        if let Some(s) = &mut self.in_file_search {
            if !s.matches.is_empty() {
                s.current = if s.current == 0 {
                    s.matches.len() - 1
                } else {
                    s.current - 1
                };
            }
        }
        self.scroll_in_file_search_to_current();
    }

    pub(crate) fn scroll_in_file_search_to_current(&mut self) {
        let Some(s) = &self.in_file_search else {
            return;
        };
        let Some(m) = s.matches.get(s.current) else {
            return;
        };
        let display_line = self.physical_to_display(m.line);
        self.scroll_line_into_view(display_line);
        self.mark_content_scrolled();
    }

    /// Re-runs in-file search against the current content source.
    pub(crate) fn refresh_in_file_search(&mut self) {
        let total = self.line_count();
        // Collect before mutably borrowing in_file_search: line_text() takes
        // &self which conflicts with the &mut borrow needed by s.refresh().
        let lines: Vec<String> = (0..total)
            .filter_map(|i| self.line_text(i).map(String::from))
            .collect();
        let Some(s) = &mut self.in_file_search else {
            return;
        };
        s.refresh(total, |i| lines.get(i).cloned());
    }

    /// Handles keyboard input while the git-history overlay is open.
    pub(super) fn handle_history_key(&mut self, key: KeyEvent) {
        let Some(ref mut h) = self.history else {
            return;
        };
        match handle_list_picker_key(h, &key) {
            OverlayKey::Activate => self.show_selected_revision(),
            OverlayKey::Close => self.history = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the theme picker overlay is open.
    pub(super) fn handle_theme_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.theme_picker else {
            return;
        };
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => self.apply_selected_theme(),
            OverlayKey::Close => self.theme_picker = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the recent-files overlay is open.
    pub(super) fn handle_recent_key(&mut self, key: KeyEvent) {
        let Some(ref mut r) = self.recent_files else {
            return;
        };
        match handle_list_picker_key(r, &key) {
            OverlayKey::Activate => self.activate_recent_selection(),
            OverlayKey::Close => self.recent_files = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the plugin manager overlay is open.
    /// Extra keys: Space toggles; j/k navigate.
    pub(super) fn handle_plugin_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.plugin_picker else {
            return;
        };
        // Extra keys first
        match key.code {
            KeyCode::Char(' ') => {
                self.toggle_plugin_picker_selection();
                return;
            }
            KeyCode::Char('k') => {
                p.selected = p.selected.saturating_sub(1);
                return;
            }
            KeyCode::Char('j') => {
                if p.selected + 1 < p.results_len() {
                    p.selected += 1;
                }
                return;
            }
            _ => {}
        }
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => self.toggle_plugin_picker_selection(),
            OverlayKey::Close => self.plugin_picker = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the inline tree filter is open.
    pub(super) fn handle_tree_filter_key(&mut self, key: KeyEvent) {
        let Some(ref mut f) = self.tree_filter else {
            return;
        };
        match handle_list_picker_key(f, &key) {
            OverlayKey::Activate | OverlayKey::Close => {
                self.tree_filter = None;
            }
            OverlayKey::Handled => {
                self.move_tree_filter_selection_to_first_match();
            }
            _ => {}
        }
    }

    /// Moves `tree_selected` to the first visible match index when the inline
    /// tree filter is active. If the query is empty or no node matches, the
    /// selection stays at index 0 (the root).
    fn move_tree_filter_selection_to_first_match(&mut self) {
        let Some(ref filter) = self.tree_filter else {
            return;
        };
        if filter.is_empty() {
            self.tree_selected = 0;
            self.scroll_tree_into_view();
            return;
        }
        let q = filter.query.to_lowercase();
        let first_match = self
            .nodes
            .iter()
            .position(|n| n.name.to_lowercase().contains(&q));
        self.tree_selected = first_match.unwrap_or(0);
        self.scroll_tree_into_view();
    }

    /// Handles keyboard input while the command palette is open.
    pub(super) fn handle_command_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.command_palette else {
            return;
        };
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => self.dispatch_command(),
            OverlayKey::Close => self.command_palette = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the go-to-line dialog is open.
    /// Extra keys: filters out the open binding so it is not appended.
    pub(super) fn handle_goto_line_key(&mut self, key: KeyEvent) {
        // Filter out the open binding key before passing to the shared dispatcher.
        if let KeyCode::Char(_) = &key.code {
            if crate::config::pressed(&self.config.keys.goto_line, &key) {
                return;
            }
        }
        let Some(ref mut g) = self.goto_line else {
            return;
        };
        match handle_list_picker_key(g, &key) {
            OverlayKey::Activate => {
                let target = self.goto_line.as_ref().and_then(|g| {
                    let q = g.query.as_str();
                    if q.is_empty() {
                        return None;
                    }
                    if let Some(offset) = q.strip_prefix('+') {
                        let n = offset.parse::<usize>().ok()?;
                        Some(self.content_scroll.saturating_add(n))
                    } else if let Some(offset) = q.strip_prefix('-') {
                        let n = offset.parse::<usize>().ok()?;
                        Some(self.content_scroll.saturating_sub(n))
                    } else {
                        let n = q.parse::<usize>().ok()?;
                        Some(n.saturating_sub(1)) // 1-indexed → 0-indexed
                    }
                });
                if let Some(line) = target {
                    self.set_content_scroll(line);
                    self.mark_content_scrolled();
                }
                self.goto_line = None;
            }
            OverlayKey::Close => {
                self.goto_line = None;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "overlay_test.rs"]
mod tests;
