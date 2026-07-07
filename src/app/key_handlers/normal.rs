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

use crossterm::event::KeyEvent;

use crate::config::{pressed_in, static_keys, BindingScope};
use crate::search::{
    CommandPalette, GotoLineState, InFileSearch, PluginPicker, SearchState, ThemePicker, TreeFilter,
};

use super::super::{App, Focus};

impl App {
    /// Handles all key events when no overlay is active. Dispatches global
    /// actions (quit, help, search, reload, etc.) and routes to tree/content
    /// handlers based on `self.focus`.
    ///
    /// `pub(crate)` rather than `pub(super)`: `app::refresh`'s
    /// `process_pending_keypress`/`preempt_pending_keypress` (protocol 3+
    /// `on_keypress` key consumption) call this directly from a sibling
    /// submodule of `key_handlers`, not just from within `key_handlers`.
    pub(crate) fn handle_normal_key(&mut self, key: KeyEvent) {
        // Clear transient status messages on the next handled keypress.
        self.status_message = None;

        if static_keys::is_close(&key) {
            if self.show_blame {
                self.show_blame = false;
                return;
            }
            if self.selection.is_some() {
                self.clear_selection();
                return;
            }
            if self.viewing_revision.is_some() {
                self.viewing_revision = None;
                if let Some(path) = self.current_file.clone() {
                    if self.git_mode {
                        self.show_working_tree_diff(&path);
                    } else {
                        self.reopen_file(&path);
                    }
                }
                return;
            }
        }
        let scope = match self.focus {
            Focus::Tree => BindingScope::Tree,
            Focus::Content => BindingScope::Content,
        };
        let k = &self.keys;
        if pressed_in(&k.quit, &key, scope) {
            self.should_quit = true;
        } else if pressed_in(&k.help, &key, scope) {
            self.show_help = !self.show_help;
        } else if pressed_in(&k.toggle_hidden, &key, scope) {
            self.show_hidden = !self.show_hidden;
            self.config.tree.show_hidden = self.show_hidden;
            self.reload();
            self.save_config();
        } else if pressed_in(&k.find_files, &key, scope) {
            self.open_file_search();
        } else if pressed_in(&k.search_files, &key, scope) {
            // A real file (`current_file`) or piped stdin content (pager
            // mode, which has no backing path but does populate `content`)
            // both count as "something to search within".
            let has_content = self.current_file.is_some() || !self.content.is_empty();
            if self.focus == Focus::Content && has_content && self.config.search.in_file_search {
                // Content focused with something loaded: open the in-file search bar.
                self.in_file_search = Some(InFileSearch::new());
            } else if self.focus == Focus::Tree {
                // Tree focused: open the inline tree name filter.
                self.tree_filter = Some(TreeFilter::new());
            } else {
                self.open_file_search();
            }
        } else if pressed_in(&k.reload, &key, scope) {
            self.viewing_revision = None;
            self.reload();
        } else if pressed_in(&k.search_content, &key, scope) {
            let root = self.root.clone();
            let changed = self.git_changed_files_set();
            let mut s = SearchState::new(
                &root,
                self.show_hidden,
                self.ignore_gitignore,
                self.config.search.context_lines,
                changed.as_ref(),
            );
            s.toggle_mode();
            if self.config.search.keep_query && !self.last_search_query.is_empty() {
                s.query = self.last_search_query.clone();
                s.refresh_now();
            }
            self.search = Some(s);
        } else if pressed_in(&k.file_history, &key, scope) {
            self.open_file_history();
        } else if pressed_in(&k.recent_files, &key, scope) {
            self.open_recent_files();
        } else if pressed_in(&k.theme_picker, &key, scope) {
            self.theme_picker = Some(ThemePicker::default());
        } else if pressed_in(&k.plugin_picker, &key, scope) {
            let entries = self.plugin_manager.plugin_entries();
            self.plugin_picker = Some(PluginPicker::new(entries));
        } else if pressed_in(&k.command_palette, &key, scope) {
            self.last_click = None;
            let (base_order, base_pinned) = crate::command_palette::ranked_base_order(
                &self.command_usage,
                self.config.palette_pin_recent,
                self.config.palette_frequent_count,
            );
            self.command_palette = Some(CommandPalette::new(&self.keys, base_order, base_pinned));
        } else if pressed_in(&k.switch_panel, &key, scope) {
            self.focus = match self.focus {
                Focus::Tree => Focus::Content,
                Focus::Content => Focus::Tree,
            };
        } else if pressed_in(&k.goto_line, &key, scope) {
            if self.focus == Focus::Content {
                self.goto_line = Some(GotoLineState::new());
            } else {
                self.set_status("go to line: switch to the content pane (Tab)");
            }
        } else if pressed_in(&k.git_mode_toggle, &key, scope) {
            self.toggle_git_mode();
        } else if pressed_in(&k.git_mode_flat_toggle, &key, scope) {
            if self.git_mode {
                self.git_mode_flat = !self.git_mode_flat;
                self.rebuild(true);
                self.try_open_selected();
                self.save_config();
            } else {
                let label = self.keys.label_for_action("git_mode_toggle");
                self.set_status(format!("flat view: only in git mode ({label})"));
            }
        } else if pressed_in(&k.open_in_editor, &key, scope) {
            self.open_in_editor();
        } else if pressed_in(&k.open_external, &key, scope) {
            self.open_external_file();
        } else if pressed_in(&k.toggle_watch, &key, scope) {
            self.auto_watch = !self.auto_watch;
            self.config.content.watch = self.auto_watch;
            self.save_config();
        } else if pressed_in(&k.copy_path, &key, scope) {
            self.copy_path_to_clipboard(false);
        } else if pressed_in(&k.copy_relative_path, &key, scope) {
            self.copy_path_to_clipboard(true);
        } else {
            match self.focus {
                Focus::Tree => self.handle_tree_key(key),
                Focus::Content => self.handle_content_key(key),
            }
        }
    }

    /// Handles navigation and expand/collapse keys when the tree panel is focused.
    pub(super) fn handle_tree_key(&mut self, key: KeyEvent) {
        let scope = BindingScope::Tree;
        let k = &self.keys;

        // When full-file blame is active, navigation keys move the content cursor
        // (active_line) instead of tree selection. Mirrors the cursor-movement
        // branches in handle_content_key, including the before/after diff that
        // marks the session dirty and the scrollbar transient — so the cursor
        // position persists across quit/reopen the same way it does when the
        // content panel is focused.
        if self.show_blame && self.has_text_cursor() {
            let scroll_before = self.content_scroll;
            let active_line_before = self.active_line;
            if pressed_in(&k.nav_up, &key, scope) {
                if self.active_line > 0 {
                    self.active_line -= 1;
                    self.scroll_active_line_into_view();
                }
            } else if pressed_in(&k.nav_down, &key, scope) {
                let max = self.display_line_count().saturating_sub(1);
                if self.active_line < max {
                    self.active_line += 1;
                    self.scroll_active_line_into_view();
                }
            } else if pressed_in(&k.content_top, &key, scope) {
                self.active_line = 0;
                self.set_content_scroll(0);
            } else if pressed_in(&k.content_bottom, &key, scope) {
                self.active_line = self.display_line_count().saturating_sub(1);
                self.scroll_active_line_into_view();
            } else if pressed_in(&k.content_page_up, &key, scope) {
                self.active_line = self.active_line.saturating_sub(self.page_rows());
                self.scroll_active_line_into_view();
            } else if pressed_in(&k.content_page_down, &key, scope) {
                let max = self.display_line_count().saturating_sub(1);
                self.active_line = (self.active_line + self.page_rows()).min(max);
                self.scroll_active_line_into_view();
            }
            if self.content_scroll != scroll_before || self.active_line != active_line_before {
                self.scroll_blame_into_view();
                self.mark_content_scrolled();
                self.mark_session_dirty();
            }
            return;
        }

        if pressed_in(&k.nav_up, &key, scope) {
            if self.tree_selected > 0 {
                self.tree_selected -= 1;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed_in(&k.nav_down, &key, scope) {
            if self.tree_selected + 1 < self.nodes.len() {
                self.tree_selected += 1;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed_in(&k.tree_expand, &key, scope) {
            self.activate_selected();
        } else if self.tree_independent_scroll && pressed_in(&k.content_page_up, &key, scope) {
            let page = self.tree_page_size();
            self.tree_scroll = self.tree_scroll.saturating_sub(page);
        } else if self.tree_independent_scroll && pressed_in(&k.content_page_down, &key, scope) {
            let page = self.tree_page_size();
            self.tree_scroll = (self.tree_scroll + page).min(self.tree_scroll_max());
        } else if pressed_in(&k.content_top, &key, scope) {
            if self.tree_independent_scroll {
                // Move only the viewport; leave the selection where it is.
                self.tree_scroll = 0;
            } else if self.tree_selected != 0 {
                self.tree_selected = 0;
                self.scroll_tree_into_view();
                self.try_open_selected();
            }
        } else if pressed_in(&k.content_bottom, &key, scope) {
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
        } else if pressed_in(&k.tree_collapse, &key, scope) {
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
                        let Some(nd) = self.nodes.get(i) else {
                            continue;
                        };
                        if nd.depth < depth {
                            self.tree_selected = i;
                            let p = nd.path.clone();
                            self.plugin_manager.on_selection_change(Some(&p));
                            break;
                        }
                    }
                }
                self.scroll_tree_into_view();
            }
        } else if pressed_in(&k.tree_collapse_all, &key, scope) {
            self.collapse_all();
        } else if pressed_in(&k.tree_expand_all, &key, scope) {
            self.expand_all();
        } else if pressed_in(&k.tree_up_dir, &key, scope) {
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
        let scope = BindingScope::Content;
        let k = &self.keys;
        let scroll_before = self.content_scroll;
        let hscroll_before = self.content_hscroll;
        let active_line_before = self.active_line;
        if pressed_in(&k.toggle_pretty_json, &key, scope) {
            if self.is_json && !self.json_pretty_lines.is_empty() {
                self.show_pretty_json = !self.show_pretty_json;
                self.set_content_scroll(0);
                self.content_hscroll = 0;
            } else if !self.is_json {
                self.set_status("pretty JSON: not a JSON file");
            } else {
                self.set_status("pretty JSON: could not parse");
            }
        } else if pressed_in(&k.toggle_raw_markdown, &key, scope) {
            if self.plugin_content_active {
                let key_str = crate::plugin::key_event_to_string(&key);
                if key_str != "M" {
                    let m_key = crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char('M'),
                        crossterm::event::KeyModifiers::SHIFT,
                    );
                    self.plugin_manager.on_keypress(&m_key);
                }
            } else {
                self.set_status(
                    "markdown render toggle: not available (current file not plugin-rendered)",
                );
            }
        } else if pressed_in(&k.toggle_blame, &key, scope) {
            if self.has_text_cursor() {
                self.show_blame = !self.show_blame;
            } else {
                self.set_status("blame: not available in a diff");
            }
        } else if self.is_diff && pressed_in(&k.toggle_diff_side_by_side, &key, scope) {
            self.diff_side_by_side = !self.diff_side_by_side;
            self.config.git.diff.side_by_side = self.diff_side_by_side;
            self.save_config();
            self.set_content_scroll(0);
            self.content_hscroll = 0;
        } else if self.is_diff && pressed_in(&k.toggle_diff_staged, &key, scope) {
            self.diff_mode = self.diff_mode.next();
            self.config.git.diff.mode = self.diff_mode;
            self.save_config();
            if let Some(path) = self.current_file.clone() {
                self.show_working_tree_diff(&path);
            }
        } else if self.is_diff && pressed_in(&k.diff_hunk_next, &key, scope) {
            self.diff_next_hunk();
        } else if self.is_diff && pressed_in(&k.diff_hunk_prev, &key, scope) {
            self.diff_prev_hunk();
        } else if !self.fold_regions.is_empty() && pressed_in(&k.fold_toggle, &key, scope) {
            // Toggle the fold region whose header is at the current scroll position.
            let phys = self.display_to_physical(self.content_scroll);
            if let Some(ri) = self.region_idx_at(phys) {
                self.toggle_fold_region(ri);
                self.mark_content_scrolled();
            }
        } else if pressed_in(&k.toggle_wrap, &key, scope) {
            self.word_wrap = !self.word_wrap;
            self.config.content.word_wrap = self.word_wrap;
            self.set_content_scroll(0);
            self.content_hscroll = 0;
            self.save_config();
        } else if pressed_in(&k.toggle_line_numbers, &key, scope) {
            self.show_line_numbers = !self.show_line_numbers;
            self.config.content.line_numbers = self.show_line_numbers;
            self.save_config();
        } else if self.has_text_cursor() && pressed_in(&k.nav_up, &key, scope) {
            // Move active line up (non-diff content).
            if self.active_line > 0 {
                self.active_line -= 1;
                self.scroll_active_line_into_view();
                self.mark_content_scrolled();
            }
        } else if self.has_text_cursor() && pressed_in(&k.nav_down, &key, scope) {
            // Move active line down (non-diff content).
            let max = self.display_line_count().saturating_sub(1);
            if self.active_line < max {
                self.active_line += 1;
                self.scroll_active_line_into_view();
                self.mark_content_scrolled();
            }
        } else if pressed_in(&k.nav_up, &key, scope) {
            // Cursorless content: fall back to scrolling.
            self.set_content_scroll(self.content_scroll.saturating_sub(1));
        } else if pressed_in(&k.nav_down, &key, scope) {
            // Cursorless content: fall back to scrolling.
            self.set_content_scroll(self.content_scroll.saturating_add(1));
        } else if pressed_in(&k.content_top, &key, scope) {
            if self.has_text_cursor() {
                self.active_line = 0;
            }
            self.set_content_scroll(0);
        } else if pressed_in(&k.content_bottom, &key, scope) {
            if self.has_text_cursor() {
                self.active_line = self.display_line_count().saturating_sub(1);
                self.scroll_active_line_into_view();
            } else {
                self.set_content_scroll(usize::MAX);
            }
        } else if pressed_in(&k.content_page_up, &key, scope) {
            if self.has_text_cursor() {
                self.active_line = self.active_line.saturating_sub(self.page_rows());
                self.scroll_active_line_into_view();
            } else {
                self.set_content_scroll(self.content_scroll.saturating_sub(self.page_rows()));
            }
        } else if pressed_in(&k.content_page_down, &key, scope) {
            if self.has_text_cursor() {
                let max = self.display_line_count().saturating_sub(1);
                self.active_line = (self.active_line + self.page_rows()).min(max);
                self.scroll_active_line_into_view();
            } else {
                self.set_content_scroll(self.content_scroll.saturating_add(self.page_rows()));
            }
        } else if !self.word_wrap && pressed_in(&k.content_left, &key, scope) {
            self.content_hscroll = self.content_hscroll.saturating_sub(4);
        } else if !self.word_wrap && pressed_in(&k.content_right, &key, scope) {
            self.content_hscroll += 4;
        } else if !self.word_wrap && pressed_in(&k.content_reset_col, &key, scope) {
            self.content_hscroll = 0;
        } else if pressed_in(&k.copy_line, &key, scope) {
            self.copy_line_or_selection();
        } else if pressed_in(&k.copy_file, &key, scope) {
            self.copy_file_content();
        } else if self.has_text_cursor() && pressed_in(&k.blame_line, &key, scope) {
            self.show_line_blame = !self.show_line_blame;
        }
        if self.content_scroll != scroll_before || self.content_hscroll != hscroll_before {
            self.mark_content_scrolled();
        }
        if self.content_scroll != scroll_before || self.active_line != active_line_before {
            self.mark_session_dirty();
        }
    }

    /// Copies the current line (or the active text selection if one exists)
    /// to the clipboard.
    pub(crate) fn copy_line_or_selection(&mut self) {
        if let Some(sel) = &self.selection {
            if !sel.is_empty() {
                let text = self.selection_text();
                if !text.is_empty() {
                    self.copy_to_clipboard(text, "selection");
                }
                return;
            }
        }
        let phys = self.display_to_physical(self.active_line);
        let text = self.line_text(phys).unwrap_or("").to_string();
        self.copy_to_clipboard(text, "line");
    }

    /// Copies the entire file content to the clipboard.
    pub(crate) fn copy_file_content(&mut self) {
        let total = self.line_count();
        let mut result = String::new();
        for i in 0..total {
            if let Some(line) = self.line_text(i) {
                if !result.is_empty() {
                    result.push('\n');
                }
                result.push_str(line);
            }
        }
        self.copy_to_clipboard(result, "file");
    }

    pub(crate) fn copy_path_to_clipboard(&mut self, relative: bool) {
        let path = match self.focus {
            Focus::Tree => self.nodes.get(self.tree_selected).map(|n| n.path.clone()),
            Focus::Content => self.current_file.clone(),
        };
        let Some(path) = path else {
            self.set_status("nothing selected");
            return;
        };
        let text = if relative {
            path.strip_prefix(&self.root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string())
        } else {
            path.display().to_string()
        };
        self.copy_to_clipboard(text, if relative { "relative path" } else { "path" });
    }

    /// Nudges `content_scroll` so `active_line` stays within the visible
    /// viewport after a cursor move. Delegates to the unified helper.
    pub(super) fn scroll_active_line_into_view(&mut self) {
        self.scroll_line_into_view(self.active_line);
    }
}
