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

mod bug_report;
mod command;
mod editor;
mod external;
mod normal;
mod overlay;
mod palette;
mod pickers;
mod tree_filter;

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

use super::{App, Overlay};
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
        match self.top_overlay() {
            Some(overlay) => self.handle_overlay_key(overlay, key),
            None => self.handle_normal_key(key),
        }
    }

    /// Routes a key to the topmost open overlay (see `app::overlay` for the
    /// stacking order, which is shared with drawing).
    fn handle_overlay_key(&mut self, overlay: Overlay, key: crossterm::event::KeyEvent) {
        match overlay {
            Overlay::Welcome => self.handle_welcome_key(key),
            Overlay::About => self.handle_about_key(key),
            Overlay::Help => self.handle_help_key(key),
            Overlay::ThemePicker => self.handle_theme_key(key),
            Overlay::ContextMenu => self.handle_context_menu_key(key),
            Overlay::MenuBar => self.handle_menu_bar_key(key),
            Overlay::CommandPalette => self.handle_command_key(key),
            Overlay::History => self.handle_history_key(key),
            Overlay::RepoLog => self.handle_repo_log_key(key),
            Overlay::RecentFiles => self.handle_recent_key(key),
            Overlay::Bookmarks => self.handle_bookmarks_key(key),
            Overlay::Search => self.handle_search_key(key),
            Overlay::InFileBar => {
                if self.json_query.is_some() {
                    self.handle_json_query_key(key);
                } else {
                    self.handle_in_file_search_key(key);
                }
            }
            Overlay::FilterBar => self.handle_filter_bar_key(key),
            Overlay::TreeFilter => self.handle_tree_filter_key(key),
            Overlay::BugReport => self.handle_bug_report_key(key),
            Overlay::RevisionPicker => self.handle_revision_key(key),
            Overlay::WorktreePicker => self.handle_worktree_key(key),
            Overlay::GotoLine => self.handle_goto_line_key(key),
        }
    }

    fn handle_welcome_key(&mut self, key: crossterm::event::KeyEvent) {
        if static_keys::is_close(&key) {
            self.show_welcome = false;
            crate::session::mark_welcome_shown();
        }
    }

    fn handle_about_key(&mut self, key: crossterm::event::KeyEvent) {
        if static_keys::is_open_release(&key) {
            self.open_release_url();
        } else if static_keys::is_modal_close(&key) {
            self.show_about = false;
        }
    }

    fn handle_help_key(&mut self, key: crossterm::event::KeyEvent) {
        if static_keys::is_modal_close(&key) {
            self.show_help = false;
            self.help_scroll.scroll = 0;
            self.help_tab = 0;
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.help_scroll.scroll_up(1),
            KeyCode::Down | KeyCode::Char('j') => self.help_scroll.scroll_down(1, usize::MAX),
            KeyCode::PageUp => self.help_scroll.scroll_up(HELP_PAGE_SIZE),
            KeyCode::PageDown => self.help_scroll.scroll_down(HELP_PAGE_SIZE, usize::MAX),
            KeyCode::Home | KeyCode::Char('g') => self.help_scroll.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.help_scroll.scroll = usize::MAX,
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
}
