//! Key handling for the revision, worktree and go-to-line overlays.

use crossterm::event::{KeyCode, KeyEvent};

use crate::list_picker::{handle_list_picker_key, OverlayKey};

use super::super::App;

impl App {
    /// Handles keyboard input while the revision picker is open.
    /// Uses the shared list-picker dispatcher for navigation, plus Enter to
    /// select (falling back to the typed query as a raw revspec when the
    /// filtered list is empty) and Esc to close. Left/Right arrows switch
    /// between tabs when the query is empty.
    pub(super) fn handle_revision_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.revision_picker else {
            return;
        };
        // Left/Right switch tabs when the query is empty.
        if p.query.is_empty() {
            match key.code {
                KeyCode::Left => {
                    p.prev_tab();
                    return;
                }
                KeyCode::Right => {
                    p.next_tab();
                    return;
                }
                _ => {}
            }
        }
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => {
                let rev = if p.results_len() > 0 && p.selected < p.results_len() {
                    p.selected_rev().map(|r| r.to_string())
                } else if !p.query.is_empty() {
                    Some(p.query.clone())
                } else {
                    None
                };
                self.revision_picker = None;
                if let Some(rev) = rev {
                    self.enter_compare_mode(rev);
                }
            }
            OverlayKey::Close => {
                self.revision_picker = None;
            }
            _ => {}
        }
    }

    /// Handles picker keys: Enter switches roots (or opens a tab when launched
    /// from that palette command), while Ctrl+Enter requests a separate tab.
    pub(super) fn handle_worktree_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyModifiers;

        let Some(ref mut picker) = self.worktree_picker else {
            return;
        };
        if key.code == KeyCode::Enter && key.modifiers.contains(KeyModifiers::CONTROL) {
            let path = picker.selected_path();
            self.worktree_picker = None;
            if let Some(path) = path {
                self.tab_action_request = Some(super::super::TabAction::OpenRoot(path));
            }
            return;
        }
        match handle_list_picker_key(picker, &key) {
            OverlayKey::Activate => {
                let (path, open_in_new_tab) = (picker.selected_path(), picker.open_in_new_tab);
                self.worktree_picker = None;
                if let Some(path) = path {
                    if open_in_new_tab {
                        self.tab_action_request = Some(super::super::TabAction::OpenRoot(path));
                    } else {
                        self.set_root(&path);
                    }
                }
            }
            OverlayKey::Close => self.worktree_picker = None,
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
                let query = self.goto_line.as_ref().map(|g| g.query.clone());
                self.goto_line = None;
                if let Some(q) = query {
                    self.goto_line_from_query(&q);
                }
            }
            OverlayKey::Close => {
                self.goto_line = None;
            }
            _ => {}
        }
    }
}
