//! Key handling when no overlay is open.
//!
//! `handle_normal_key` dispatches the main editing surface: global actions
//! (quit, help, toggle hidden files, reload, open the search/history/theme/
//! recent-files/command overlays, theme cycling, git-mode toggles) plus
//! focus-specific movement that it forwards to the tree or content handlers
//! based on `App::focus`. It also handles the entry into visual-line mode and
//! clearing an active text selection. This is the busiest handler in the app;
//! keep new global keybindings here and route panel-specific motion to the
//! tree/content helpers rather than inlining it.

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::pressed;
use crate::search::{
    CommandPalette, GotoLineState, InFileSearch, PluginPicker, SearchState, ThemePicker, TreeFilter,
};

use super::super::{App, Focus};

impl App {
    /// Handles all key events when no overlay is active. Dispatches global
    /// actions (quit, help, search, reload, etc.) and routes to tree/content
    /// handlers based on `self.focus`.
    pub(super) fn handle_normal_key(&mut self, key: KeyEvent) {
        // Clear transient status messages on the next handled keypress.
        self.status_message = None;

        if self.visual_line.is_some() && self.focus == Focus::Content {
            self.handle_visual_line_key(key);
            return;
        }
        if key.code == KeyCode::Esc {
            if self.show_line_blame {
                self.show_line_blame = false;
                return;
            }
            if self.selection.is_some() {
                self.clear_selection();
                return;
            }
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
                // Content focused with an open file: open the in-file search bar.
                self.in_file_search = Some(InFileSearch::new());
            } else if self.focus == Focus::Tree {
                // Tree focused: open the inline tree name filter.
                self.tree_filter = Some(TreeFilter::new());
            } else {
                // Content focused but no file open (or in-file search disabled):
                // fall back to the full filesystem search picker.
                let root = self.root.clone();
                let mut s = SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search_context_lines,
                );
                if self.config.keep_search_query && !self.last_search_query.is_empty() {
                    s.query = self.last_search_query.clone();
                    s.refresh_now();
                }
                self.search = Some(s);
            }
        } else if pressed(&k.reload, &key) {
            self.reload();
        } else if pressed(&k.search_content, &key) {
            let root = self.root.clone();
            let mut s = SearchState::new(
                &root,
                self.show_hidden,
                self.ignore_gitignore,
                self.config.search_context_lines,
            );
            s.toggle_mode();
            if self.config.keep_search_query && !self.last_search_query.is_empty() {
                s.query = self.last_search_query.clone();
                s.refresh_now();
            }
            self.search = Some(s);
        } else if pressed(&k.file_history, &key) {
            self.open_file_history();
        } else if pressed(&k.recent_files, &key) {
            self.open_recent_files();
        } else if pressed(&k.theme_picker, &key) {
            self.theme_picker = Some(ThemePicker::default());
        } else if pressed(&k.plugin_picker, &key) {
            let entries = self.plugin_manager.plugin_entries();
            self.plugin_picker = Some(PluginPicker::new(entries));
        } else if pressed(&k.command_palette, &key) {
            self.last_click = None;
            self.command_palette = Some(CommandPalette::new(&self.keys));
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
                self.rebuild(true);
                self.try_open_selected();
                self.save_config();
            } else {
                self.status_message = Some("flat view: only in git mode (Ctrl+G)".into());
            }
        } else if pressed(&k.open_in_editor, &key) {
            self.open_in_editor();
        } else if pressed(&k.toggle_watch, &key) {
            self.auto_watch = !self.auto_watch;
            self.config.watch = self.auto_watch;
            self.save_config();
        } else if pressed(&k.copy_path, &key) {
            self.copy_path_to_clipboard(false);
        } else if pressed(&k.copy_relative_path, &key) {
            self.copy_path_to_clipboard(true);
        } else if pressed(&k.goto_line, &key) {
            if self.focus == Focus::Content {
                self.goto_line = Some(GotoLineState::new());
            } else {
                self.status_message = Some("go to line: switch to the content pane (Tab)".into());
            }
        } else {
            match self.focus {
                Focus::Tree => self.handle_tree_key(key),
                Focus::Content => self.handle_content_key(key),
            }
        }
    }

    /// Handles navigation and expand/collapse keys when the tree panel is focused.
    pub(super) fn handle_tree_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        if pressed(&k.nav_up, &key) {
            if self.tree_selected > 0 {
                self.tree_selected -= 1;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed(&k.nav_down, &key) {
            if self.tree_selected + 1 < self.nodes.len() {
                self.tree_selected += 1;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed(&k.tree_expand, &key) {
            self.activate_selected();
        } else if self.tree_independent_scroll && pressed(&k.content_page_up, &key) {
            let page = self.tree_page_size();
            self.tree_scroll = self.tree_scroll.saturating_sub(page);
        } else if self.tree_independent_scroll && pressed(&k.content_page_down, &key) {
            let page = self.tree_page_size();
            self.tree_scroll = (self.tree_scroll + page).min(self.tree_scroll_max());
        } else if pressed(&k.content_top, &key) {
            if self.tree_independent_scroll {
                // Move only the viewport; leave the selection where it is.
                self.tree_scroll = 0;
            } else if self.tree_selected != 0 {
                self.tree_selected = 0;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed(&k.content_bottom, &key) {
            if self.tree_independent_scroll {
                self.tree_scroll = self.tree_scroll_max();
            } else {
                let last = self.nodes.len().saturating_sub(1);
                if self.tree_selected != last {
                    self.tree_selected = last;
                    self.scroll_tree_into_view();
                    self.try_open_selected();
                }
            }
        } else if pressed(&k.tree_collapse, &key) {
            if let Some(node) = self.nodes.get(self.tree_selected) {
                let depth = node.depth;
                let path = node.path.clone();
                let is_dir = node.is_dir;

                if is_dir && self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                    self.mark_session_dirty();
                    self.rebuild(true);
                } else if depth > 0 {
                    for i in (0..self.tree_selected).rev() {
                        if self.nodes[i].depth < depth {
                            self.tree_selected = i;
                            let p = self.nodes[i].path.clone();
                            self.plugin_manager.on_selection_change(Some(&p));
                            break;
                        }
                    }
                }
                self.scroll_tree_into_view();
            }
        } else if pressed(&k.tree_collapse_all, &key) {
            self.collapse_all();
        } else if pressed(&k.tree_expand_all, &key) {
            self.expand_all();
        } else if pressed(&k.tree_up_dir, &key) {
            self.tree_up_dir();
        }
    }

    /// Number of tree rows that fit in the visible viewport, from the geometry
    /// captured during the last render. Falls back to 1 before the first draw.
    fn tree_page_size(&self) -> usize {
        (self.tree_area.height as usize).max(1)
    }

    /// Largest valid `tree_scroll` value so the last row can sit at the bottom
    /// of the viewport without scrolling past the end of the tree.
    pub(crate) fn tree_scroll_max(&self) -> usize {
        self.nodes.len().saturating_sub(self.tree_page_size())
    }

    /// Nudges `tree_scroll` so the current selection stays within the viewport
    /// after a cursor move.
    pub(crate) fn scroll_tree_into_view(&mut self) {
        let height = self.tree_page_size();
        if self.tree_selected < self.tree_scroll {
            self.tree_scroll = self.tree_selected;
        } else if self.tree_selected >= self.tree_scroll + height {
            self.tree_scroll = self.tree_selected + 1 - height;
        }
    }

    /// Handles scrolling, wrapping, and markdown-raw toggle keys when the
    /// content panel is focused.
    pub(super) fn handle_content_key(&mut self, key: KeyEvent) {
        let k = &self.keys;
        let scroll_before = self.content_scroll;
        let hscroll_before = self.content_hscroll;
        let active_line_before = self.active_line;
        if pressed(&k.toggle_raw_markdown, &key) {
            if self.is_markdown {
                self.show_raw_markdown = !self.show_raw_markdown;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            } else {
                self.status_message = Some("raw toggle: not a markdown file".into());
            }
        } else if pressed(&k.toggle_pretty_json, &key) {
            if self.is_json && !self.json_pretty_lines.is_empty() {
                self.show_pretty_json = !self.show_pretty_json;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            } else if !self.is_json {
                self.status_message = Some("pretty JSON: not a JSON file".into());
            } else {
                self.status_message = Some("pretty JSON: could not parse".into());
            }
        } else if pressed(&k.toggle_blame, &key) {
            if !self.is_diff {
                self.show_blame = !self.show_blame;
            } else {
                self.status_message = Some("blame: not available in a diff".into());
            }
        } else if !self.is_diff
            && self.current_file.is_some()
            && pressed(&k.visual_line_toggle, &key)
        {
            self.enter_visual_line();
        } else if self.is_diff && pressed(&k.toggle_diff_side_by_side, &key) {
            self.diff_side_by_side = !self.diff_side_by_side;
            self.content_scroll = 0;
            self.content_hscroll = 0;
        } else if self.is_diff && pressed(&k.toggle_diff_staged, &key) {
            self.diff_mode = self.diff_mode.next();
            if let Some(path) = self.current_file.clone() {
                self.show_working_tree_diff(&path);
            }
        } else if self.is_diff && pressed(&k.diff_hunk_next, &key) {
            self.diff_next_hunk();
        } else if self.is_diff && pressed(&k.diff_hunk_prev, &key) {
            self.diff_prev_hunk();
        } else if !self.fold_regions.is_empty() && pressed(&k.fold_toggle, &key) {
            // Toggle the fold region whose header is at the current scroll position.
            let phys = self.display_to_physical(self.content_scroll);
            if let Some(ri) = self.region_idx_at(phys) {
                self.toggle_fold_region(ri);
                self.mark_content_scrolled();
            }
        } else if pressed(&k.toggle_wrap, &key) {
            self.word_wrap = !self.word_wrap;
            self.config.word_wrap = self.word_wrap;
            self.content_scroll = 0;
            self.content_hscroll = 0;
            self.save_config();
        } else if pressed(&k.toggle_line_numbers, &key) {
            self.show_line_numbers = !self.show_line_numbers;
            self.config.line_numbers = self.show_line_numbers;
            self.save_config();
        } else if !self.is_diff && pressed(&k.nav_up, &key) {
            // Move active line up (non-diff content).
            if self.active_line > 0 {
                self.active_line -= 1;
                self.scroll_active_line_into_view();
                self.mark_content_scrolled();
            }
        } else if !self.is_diff && pressed(&k.nav_down, &key) {
            // Move active line down (non-diff content).
            let max = self.display_line_count().saturating_sub(1);
            if self.active_line < max {
                self.active_line += 1;
                self.scroll_active_line_into_view();
                self.mark_content_scrolled();
            }
        } else if pressed(&k.nav_up, &key) {
            // Diff: fall back to scrolling.
            self.content_scroll = self.content_scroll.saturating_sub(1);
        } else if pressed(&k.nav_down, &key) {
            // Diff: fall back to scrolling.
            let max = self.content_scroll_max();
            if self.content_scroll < max {
                self.content_scroll += 1;
            }
        } else if pressed(&k.content_top, &key) {
            if !self.is_diff {
                self.active_line = 0;
            }
            self.content_scroll = 0;
        } else if pressed(&k.content_bottom, &key) {
            if !self.is_diff {
                self.active_line = self.display_line_count().saturating_sub(1);
                self.scroll_active_line_into_view();
            } else {
                self.content_scroll = self.content_scroll_max();
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
        } else if !self.word_wrap && pressed(&k.content_reset_col, &key) {
            self.content_hscroll = 0;
        } else if !self.is_diff && pressed(&k.blame_line, &key) {
            self.show_line_blame = !self.show_line_blame;
        }
        if self.content_scroll != scroll_before || self.content_hscroll != hscroll_before {
            self.mark_content_scrolled();
        }
        if self.content_scroll != scroll_before || self.active_line != active_line_before {
            self.mark_session_dirty();
        }
    }

    pub(crate) fn copy_path_to_clipboard(&mut self, relative: bool) {
        let Some(path) = self.current_file.as_ref() else {
            self.status_message = Some("no file selected".into());
            return;
        };
        let text = if relative {
            path.strip_prefix(&self.root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string())
        } else {
            path.display().to_string()
        };
        let mut clipboard = match arboard::Clipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                self.status_message = Some(format!("clipboard error: {e}"));
                return;
            }
        };
        match clipboard.set_text(text) {
            Ok(()) => self.status_message = Some("path copied".into()),
            Err(e) => self.status_message = Some(format!("clipboard error: {e}")),
        }
    }

    /// Nudges `content_scroll` so `active_line` stays within the visible
    /// viewport after a cursor move.
    fn scroll_active_line_into_view(&mut self) {
        let view_height = (self.content_area.height as usize).max(1);
        if self.active_line < self.content_scroll {
            self.content_scroll = self.active_line;
        } else if self.active_line >= self.content_scroll + view_height {
            self.content_scroll = self
                .active_line
                .saturating_sub(view_height)
                .saturating_add(1);
        }
    }
}
