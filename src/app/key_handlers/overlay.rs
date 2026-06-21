//! Key handling for the fuzzy-picker overlays.
//!
//! The search, history, theme-picker, recent-files, and command-palette overlays
//! share a common interaction shape - type to filter, up/down to move the
//! selection, Enter to act, Esc to close - and this module implements the key
//! handling for each. `handle_search_key` and its siblings push/pop query
//! characters and call the picker's `refresh()` to re-score the filtered list,
//! toggle search mode with Tab where applicable, and on Enter hand off to the
//! relevant `App` open action. Closing an overlay clears its state and returns
//! focus to the underlying panel.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::App;

impl App {
    /// Handles keyboard input while the search overlay is open: typing
    /// characters, backspace, up/down navigation, Tab to toggle mode,
    /// Enter to open, Esc to close.
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Some(s) = &self.search {
                    self.last_search_query = s.query.clone();
                }
                self.search = None;
            }
            KeyCode::Tab => {
                if let Some(s) = &mut self.search {
                    s.toggle_mode();
                }
            }
            KeyCode::Enter => self.activate_search_selection(),
            KeyCode::Up => {
                if let Some(s) = &mut self.search {
                    s.selected = s.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(s) = &mut self.search {
                    if s.selected + 1 < s.results_len() {
                        s.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(s) = &mut self.search {
                    s.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(s) = &mut self.search {
                    s.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while in-file search is active: typing chars,
    /// backspace, n/N for next/prev, Esc/Enter to close.
    pub(super) fn handle_in_file_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.in_file_search = None;
            }
            KeyCode::Char('n') => {
                self.in_file_search_next();
            }
            KeyCode::Char('N') | KeyCode::Char('P') => {
                self.in_file_search_prev();
            }
            KeyCode::Tab => {
                self.in_file_search_next();
            }
            KeyCode::BackTab => {
                self.in_file_search_prev();
            }
            KeyCode::Char('p') if key.modifiers.intersects(KeyModifiers::CONTROL) => {
                self.in_file_search_prev();
            }
            KeyCode::Backspace => {
                if let Some(ref mut s) = self.in_file_search {
                    s.pop();
                }
                self.refresh_in_file_search();
                self.scroll_in_file_search_to_current();
            }
            KeyCode::Char(c) => {
                if let Some(ref mut s) = self.in_file_search {
                    s.push(c);
                }
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
        let view_height = (self.content_area.height as usize).max(1);
        let Some(s) = &self.in_file_search else {
            return;
        };
        let Some(m) = s.matches.get(s.current) else {
            return;
        };
        let display_line = self.physical_to_display(m.line);
        if display_line < self.content_scroll {
            self.content_scroll = display_line;
        } else if display_line >= self.content_scroll + view_height {
            self.content_scroll = display_line.saturating_sub(view_height).saturating_add(1);
        }
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
        match key.code {
            KeyCode::Esc => self.history = None,
            KeyCode::Enter => self.show_selected_revision(),
            KeyCode::Up => {
                if let Some(h) = &mut self.history {
                    h.selected = h.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(h) = &mut self.history {
                    if h.selected + 1 < h.results_len() {
                        h.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(h) = &mut self.history {
                    h.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(h) = &mut self.history {
                    h.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the theme picker overlay is open.
    pub(super) fn handle_theme_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.theme_picker = None,
            KeyCode::Enter => self.apply_selected_theme(),
            KeyCode::Up => {
                if let Some(p) = &mut self.theme_picker {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(p) = &mut self.theme_picker {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = &mut self.theme_picker {
                    p.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = &mut self.theme_picker {
                    p.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the recent-files overlay is open.
    pub(super) fn handle_recent_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.recent_files = None,
            KeyCode::Enter => self.activate_recent_selection(),
            KeyCode::Up => {
                if let Some(r) = &mut self.recent_files {
                    r.selected = r.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(r) = &mut self.recent_files {
                    if r.selected + 1 < r.results_len() {
                        r.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(r) = &mut self.recent_files {
                    r.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(r) = &mut self.recent_files {
                    r.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the plugin manager overlay is open.
    /// Up/Down navigates the list; Space or Enter toggles the selected plugin;
    /// Esc closes without any further action.
    pub(super) fn handle_plugin_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.plugin_picker = None,
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_plugin_picker_selection(),
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(p) = &mut self.plugin_picker {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(p) = &mut self.plugin_picker {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the inline tree filter is open.
    ///
    /// Typing characters narrows the tree to nodes whose names contain the
    /// query (case-insensitive); Backspace removes the last character; Esc or
    /// Enter dismisses the filter bar and resets the filter.
    pub(super) fn handle_tree_filter_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.tree_filter = None;
            }
            KeyCode::Backspace => {
                if let Some(ref mut f) = self.tree_filter {
                    f.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut f) = self.tree_filter {
                    f.push(c);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the command palette is open: typing
    /// characters, backspace, up/down navigation, Enter to execute, Esc to close.
    pub(super) fn handle_command_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.command_palette = None;
            }
            KeyCode::Enter => self.dispatch_command(),
            KeyCode::Up => {
                if let Some(p) = &mut self.command_palette {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(p) = &mut self.command_palette {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(p) = &mut self.command_palette {
                    p.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Some(p) = &mut self.command_palette {
                    p.push(c);
                }
            }
            _ => {}
        }
    }
}
