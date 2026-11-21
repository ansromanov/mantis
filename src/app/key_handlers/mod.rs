//! Keyboard dispatch entry point for `App`.
//!
//! `handle_key` is the single funnel for every key event. It first filters out
//! key-release events (Windows reports both press and release) so each action
//! fires once, then routes by precedence: modal overlays (about, help, menu,
//! theme picker, history, recent files, command palette, search) consume input first,
//! and only when none is active does control fall through to the normal
//! tree/content handler. The actual handling lives in the sibling submodules
//! wired up here: `normal` (no overlay), `overlay` (search/picker editing),
//! and `editor` (command dispatch and
//! external-editor suspend/resume).

mod editor;
mod normal;
mod overlay;

#[cfg(test)]
#[path = "editor_test.rs"]
mod editor_tests;
#[cfg(test)]
#[path = "mod_test.rs"]
mod mod_tests;
#[cfg(test)]
#[path = "normal_test.rs"]
mod normal_tests;

use crossterm::event::KeyCode;

use super::App;
use crate::config::static_keys;

/// Page size for help popup scrolling (matches `handle_list_picker_key`).
const HELP_PAGE_SIZE: usize = 10;

impl App {
    /// Dispatches a key event. Overlays (help, theme, history, search) are
    /// checked first; otherwise normal tree/content key handling applies.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyEventKind;

        // The Windows console backend reports both a Press and a Release event
        // for a single physical key press (Unix only reports Press unless the
        // kitty protocol is enabled). Ignore Release so every action runs once
        // rather than twice. `Repeat` is kept so held keys still navigate.
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.pause_follow_for_input();
        // Ctrl+L is a recovery hatch when a terminal frame is corrupted.
        if key.code == KeyCode::Char('l')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            self.needs_clear = true;
            self.set_status("Full redraw requested");
            return;
        }
        // Notify plugins of each keypress *only* in normal mode (no overlay
        // active) so search/picker input is not broadcast. Plugins receive
        // the key as a readable string: "q", "ctrl+c", "Enter", etc.
        if self.show_telemetry_notice {
            if static_keys::is_modal_close(&key) {
                self.show_telemetry_notice = false;
            }
            return;
        }
        if self.show_welcome {
            if static_keys::is_close(&key) {
                self.show_welcome = false;
                crate::session::mark_welcome_shown();
            }
            return;
        }
        if self.show_about {
            if static_keys::is_open_release(&key) {
                self.open_release_url();
            } else if static_keys::is_modal_close(&key) {
                self.show_about = false;
            }
            return;
        }
        if self.show_help {
            if static_keys::is_modal_close(&key) {
                self.show_help = false;
                self.help_scroll.scroll = 0;
                self.help_tab = 0;
            } else {
                match key.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.help_scroll.scroll_up(1);
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        self.help_scroll.scroll_down(1, usize::MAX);
                    }
                    KeyCode::PageUp => {
                        self.help_scroll.scroll_up(HELP_PAGE_SIZE);
                    }
                    KeyCode::PageDown => {
                        self.help_scroll.scroll_down(HELP_PAGE_SIZE, usize::MAX);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        self.help_scroll.scroll = 0;
                    }
                    KeyCode::End | KeyCode::Char('G') => {
                        self.help_scroll.scroll = usize::MAX;
                    }
                    KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => {
                        self.help_tab = (self.help_tab + 1) % crate::ui::popups::help_tabs().len();
                        self.help_scroll.scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                        self.help_tab = if self.help_tab == 0 {
                            crate::ui::popups::help_tabs().len() - 1
                        } else {
                            self.help_tab - 1
                        };
                        self.help_scroll.scroll = 0;
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_key(key);
            return;
        }
        if self.context_menu.is_some() {
            self.handle_context_menu_key(key);
        } else if self.menu_bar_state.is_some() {
            self.handle_menu_bar_key(key);
        } else if self.command_palette.is_some() {
            self.handle_command_key(key);
        } else if self.history.is_some() {
            self.handle_history_key(key);
        } else if self.repo_log.is_some() {
            self.handle_repo_log_key(key);
        } else if self.recent_files.is_some() {
            self.handle_recent_key(key);
        } else if self.bookmarks.is_some() {
            self.handle_bookmarks_key(key);
        } else if self.search.is_some() {
            self.handle_search_key(key);
        } else if self.json_query.is_some() {
            self.handle_json_query_key(key);
        } else if self.in_file_search_open {
            self.handle_in_file_search_key(key);
        } else if self.filter_bar.is_some() {
            self.handle_filter_bar_key(key);
        } else if self.tree_filter.is_some() {
            self.handle_tree_filter_key(key);
        } else if self.bug_report.is_some() {
            self.handle_bug_report_key(key);
        } else if self.revision_picker.is_some() {
            self.handle_revision_key(key);
        } else if self.worktree_picker.is_some() {
            self.handle_worktree_key(key);
        } else if self.goto_line.is_some() {
            self.handle_goto_line_key(key);
        } else {
            self.handle_normal_key(key);
        }
    }
}
