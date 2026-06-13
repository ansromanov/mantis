use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::pressed;
use crate::highlight::Highlighter;
use crate::search::{CommandPalette, InFileSearch, SearchState, ThemePicker};
use crate::theme::{Theme, ThemeConfig};

use super::{diff_line_style, App, Focus};

impl App {
    /// Dispatches a key event. Overlays (help, theme, history, search) are
    /// checked first; otherwise normal tree/content key handling applies.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if self.show_help {
            if matches!(
                key.code,
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('q')
            ) {
                self.show_help = false;
            }
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_key(key);
        } else if self.command_palette.is_some() {
            self.handle_command_key(key);
        } else if self.history.is_some() {
            self.handle_history_key(key);
        } else if self.search.is_some() {
            self.handle_search_key(key);
        } else if self.in_file_search.is_some() {
            self.handle_in_file_search_key(key);
        } else {
            self.handle_normal_key(key);
        }
    }

    /// Handles keyboard input while the search overlay is open: typing
    /// characters, backspace, up/down navigation, Tab to toggle mode,
    /// Enter to open, Esc to close.
    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
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
    fn handle_in_file_search_key(&mut self, key: KeyEvent) {
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
                if let Some(s) = &mut self.in_file_search {
                    s.pop(&self.content);
                }
                self.scroll_in_file_search_to_current();
            }
            KeyCode::Char(c) => {
                if let Some(s) = &mut self.in_file_search {
                    s.push(c, &self.content);
                }
                self.scroll_in_file_search_to_current();
            }
            _ => {}
        }
    }

    fn in_file_search_next(&mut self) {
        if let Some(s) = &mut self.in_file_search {
            if !s.matches.is_empty() {
                s.current = (s.current + 1) % s.matches.len();
            }
        }
        self.scroll_in_file_search_to_current();
    }

    fn in_file_search_prev(&mut self) {
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

    fn scroll_in_file_search_to_current(&mut self) {
        let view_height = (self.content_area.height as usize).max(1);
        let Some(s) = &self.in_file_search else {
            return;
        };
        let Some(m) = s.matches.get(s.current) else {
            return;
        };
        if m.line < self.content_scroll {
            self.content_scroll = m.line;
        } else if m.line >= self.content_scroll + view_height {
            self.content_scroll = m.line.saturating_sub(view_height).saturating_add(1);
        }
        self.mark_content_scrolled();
    }

    /// Handles keyboard input while the git-history overlay is open.
    fn handle_history_key(&mut self, key: KeyEvent) {
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
    fn handle_theme_key(&mut self, key: KeyEvent) {
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

    /// Handles keyboard input while the command palette is open: typing
    /// characters, backspace, up/down navigation, Enter to execute, Esc to close.
    fn handle_command_key(&mut self, key: KeyEvent) {
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

    /// Executes the selected command from the palette and closes it.
    pub(super) fn dispatch_command(&mut self) {
        let action_id = self
            .command_palette
            .as_ref()
            .and_then(|p| p.selected_command().map(|c| c.action_id));
        self.command_palette = None;
        match action_id {
            Some("toggle_help") => self.show_help = !self.show_help,
            Some("toggle_hidden") => {
                self.show_hidden = !self.show_hidden;
                self.config.show_hidden = self.show_hidden;
                self.reload();
                self.save_config();
            }
            Some("open_file_search") => {
                let root = self.root.clone();
                self.search = Some(SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                ));
            }
            Some("open_content_search") => {
                let root = self.root.clone();
                let mut s = SearchState::new(&root, self.show_hidden, self.ignore_gitignore);
                s.toggle_mode();
                self.search = Some(s);
            }
            Some("reload") => self.reload(),
            Some("open_file_history") => self.open_file_history(),
            Some("open_theme_picker") => {
                self.theme_picker = Some(ThemePicker::default());
            }
            Some("toggle_git_mode") => self.toggle_git_mode(),
            Some("toggle_git_flat") => {
                if self.git_mode {
                    self.git_mode_flat = !self.git_mode_flat;
                    self.config.git_mode_flat = self.git_mode_flat;
                    self.rebuild();
                    self.try_open_selected();
                    self.save_config();
                }
            }
            Some("toggle_word_wrap") => {
                self.word_wrap = !self.word_wrap;
                self.config.word_wrap = self.word_wrap;
                self.content_scroll = 0;
                self.content_hscroll = 0;
                self.save_config();
            }
            Some("toggle_raw_markdown") if self.is_markdown => {
                self.show_raw_markdown = !self.show_raw_markdown;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            }
            _ => {}
        }
    }

    /// Applies the theme selected in the picker, saves it to config, and
    /// closes the overlay.
    pub(super) fn apply_selected_theme(&mut self) {
        let name = self.theme_picker.as_ref().and_then(|p| p.selected_name());
        self.theme_picker = None;
        if let Some(name) = name {
            if let Some(theme) = Theme::preset(name) {
                self.apply_theme(theme);
                self.config.theme = ThemeConfig::from_preset(name);
                self.save_config();
            }
        }
    }

    /// Switches the active theme and re-renders the current view with it,
    /// preserving scroll position.
    fn apply_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.highlighter = Highlighter::new(&self.theme.syntax);
        if self.is_diff {
            self.highlighted = self
                .content
                .iter()
                .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
                .collect();
        } else if let Some(path) = self.current_file.clone() {
            self.reopen_file(&path);
        }
    }

    /// Handles all key events when no overlay is active. Dispatches global
    /// actions (quit, help, search, reload, etc.) and routes to tree/content
    /// handlers based on `self.focus`.
    fn handle_normal_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc && self.selection.is_some() {
            self.clear_selection();
            return;
        }
        let k = &self.keys;
        if pressed(&k.quit, &key) {
            self.should_quit = true;
        } else if pressed(&k.help, &key) {
            self.show_help = !self.show_help;
        } else if pressed(&k.toggle_hidden, &key) {
            self.show_hidden = !self.show_hidden;
            self.config.show_hidden = self.show_hidden;
            self.reload();
            self.save_config();
        } else if pressed(&k.search_files, &key) {
            if self.focus == Focus::Content
                && self.current_file.is_some()
                && self.config.in_file_search
            {
                self.in_file_search = Some(InFileSearch::new());
            } else {
                let root = self.root.clone();
                self.search = Some(SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                ));
            }
        } else if pressed(&k.reload, &key) {
            self.reload();
        } else if pressed(&k.search_content, &key) {
            let root = self.root.clone();
            let mut s = SearchState::new(&root, self.show_hidden, self.ignore_gitignore);
            s.toggle_mode();
            self.search = Some(s);
        } else if pressed(&k.file_history, &key) {
            self.open_file_history();
        } else if pressed(&k.theme_picker, &key) {
            self.theme_picker = Some(ThemePicker::default());
        } else if pressed(&k.command_palette, &key) {
            self.command_palette = Some(CommandPalette::default());
        } else if pressed(&k.switch_panel, &key) {
            self.focus = match self.focus {
                Focus::Tree => Focus::Content,
                Focus::Content => Focus::Tree,
            };
        } else if pressed(&k.git_mode_toggle, &key) {
            self.toggle_git_mode();
        } else if pressed(&k.git_mode_flat_toggle, &key) {
            if self.git_mode {
                self.git_mode_flat = !self.git_mode_flat;
                self.config.git_mode_flat = self.git_mode_flat;
                self.rebuild();
                self.try_open_selected();
                self.save_config();
            }
        } else {
            match self.focus {
                Focus::Tree => self.handle_tree_key(key),
                Focus::Content => self.handle_content_key(key),
            }
        }
    }

    /// Handles navigation and expand/collapse keys when the tree panel is focused.
    fn handle_tree_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if pressed(&k.nav_up, &key) {
            if self.tree_selected > 0 {
                self.tree_selected -= 1;
                self.try_open_selected();
            }
        } else if pressed(&k.nav_down, &key) {
            if self.tree_selected + 1 < self.nodes.len() {
                self.tree_selected += 1;
                self.try_open_selected();
            }
        } else if pressed(&k.tree_expand, &key) {
            self.activate_selected();
        } else if pressed(&k.tree_collapse, &key) {
            if let Some(node) = self.nodes.get(self.tree_selected) {
                let depth = node.depth;
                let path = node.path.clone();
                let is_dir = node.is_dir;

                if is_dir && self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                    self.rebuild();
                } else if depth > 0 {
                    for i in (0..self.tree_selected).rev() {
                        if self.nodes[i].depth < depth {
                            self.tree_selected = i;
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Handles scrolling, wrapping, and markdown-raw toggle keys when the
    /// content panel is focused.
    fn handle_content_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        let scroll_before = self.content_scroll;
        let hscroll_before = self.content_hscroll;
        if self.is_markdown && pressed(&k.toggle_raw_markdown, &key) {
            self.show_raw_markdown = !self.show_raw_markdown;
            self.content_scroll = 0;
            self.content_hscroll = 0;
        } else if pressed(&k.toggle_wrap, &key) {
            self.word_wrap = !self.word_wrap;
            self.config.word_wrap = self.word_wrap;
            self.content_scroll = 0;
            self.content_hscroll = 0;
            self.save_config();
        } else if pressed(&k.nav_up, &key) {
            self.content_scroll = self.content_scroll.saturating_sub(1);
        } else if pressed(&k.nav_down, &key) {
            let max = self.content_scroll_max();
            if self.content_scroll < max {
                self.content_scroll += 1;
            }
        } else if pressed(&k.content_page_up, &key) {
            self.content_scroll = self.content_scroll.saturating_sub(20);
        } else if pressed(&k.content_page_down, &key) {
            let max = self.content_scroll_max();
            self.content_scroll = (self.content_scroll + 20).min(max);
        } else if !self.word_wrap && pressed(&k.content_left, &key) {
            self.content_hscroll = self.content_hscroll.saturating_sub(4);
        } else if !self.word_wrap && pressed(&k.content_right, &key) {
            self.content_hscroll += 4;
        } else if pressed(&k.content_top, &key) {
            self.content_scroll = 0;
        } else if pressed(&k.content_bottom, &key) {
            self.content_scroll = self.content_scroll_max();
        } else if !self.word_wrap && pressed(&k.content_reset_col, &key) {
            self.content_hscroll = 0;
        }
        if self.content_scroll != scroll_before || self.content_hscroll != hscroll_before {
            self.mark_content_scrolled();
        }
    }
}
