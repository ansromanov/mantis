//! Key handling for the fuzzy-picker overlays.
//!
//! Each overlay handler is a thin wrapper around the shared
//! [`handle_list_picker_key`] dispatcher: handle extra keys first, then
//! fall through to the dispatcher and map `Activate`/`Close` to the
//! overlay-specific action.

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::static_keys;
use crate::list_picker::{handle_list_picker_key, OverlayKey};
use crate::theme::Theme;
use serde_json::Value;

use super::super::App;

impl App {
    pub(super) fn open_json_query(&mut self) {
        if !self.is_json && !self.is_jsonl {
            self.set_status("JSON query: requires JSON or JSONL content");
            return;
        }
        self.json_query_original = self.content.clone();
        self.json_query = Some(String::new());
    }

    fn apply_json_query(&mut self) {
        let Some(input) = self.json_query.clone() else {
            return;
        };
        let parsed = match crate::json_query::parse(&input) {
            Ok(query) => query,
            Err(error) => {
                self.set_status(format!("JSON query: {error}"));
                return;
            }
        };
        let values: Vec<Value> = if self.is_jsonl {
            self.jsonl_source
                .iter()
                .filter_map(|line| serde_json::from_str(line).ok())
                .flat_map(|value| crate::json_query::evaluate(&parsed, &value))
                .collect()
        } else {
            let source = self.json_pretty_text.join("\n");
            serde_json::from_str(&source)
                .ok()
                .map_or_else(Vec::new, |value| {
                    crate::json_query::evaluate(&parsed, &value)
                })
        };
        self.content = values
            .iter()
            .flat_map(|value| {
                serde_json::to_string_pretty(value)
                    .unwrap_or_else(|_| value.to_string())
                    .lines()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .collect();
        if self.content.is_empty() {
            self.content.push("[query returned no results]".into());
        }
        self.highlighted = self.highlighter.highlight(
            self.current_file
                .as_deref()
                .unwrap_or(std::path::Path::new("")),
            &self.content,
        );
        self.active_line = 0;
        self.set_content_scroll(0);
    }

    pub(super) fn handle_json_query_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.content = std::mem::take(&mut self.json_query_original);
            self.json_query = None;
            self.highlighted = self.highlighter.highlight(
                self.current_file
                    .as_deref()
                    .unwrap_or(std::path::Path::new("")),
                &self.content,
            );
            self.clamp_content_scroll();
            return;
        }
        match key.code {
            KeyCode::Backspace => {
                if let Some(query) = &mut self.json_query {
                    query.pop();
                }
                if self.json_query.as_deref() == Some("") {
                    self.content = self.json_query_original.clone();
                    self.highlighted = self.highlighter.highlight(
                        self.current_file
                            .as_deref()
                            .unwrap_or(std::path::Path::new("")),
                        &self.content,
                    );
                } else {
                    self.apply_json_query();
                }
            }
            KeyCode::Char(c) => {
                if let Some(query) = &mut self.json_query {
                    query.push(c);
                }
                self.apply_json_query();
            }
            KeyCode::Enter => {}
            _ => {}
        }
    }

    /// Handles keyboard input while in-file search is active.
    /// Extra keys: n/N/Tab/BackTab/Ctrl-p/Up/Down navigate matches.
    pub(super) fn handle_in_file_search_key(&mut self, key: KeyEvent) {
        // Extra keys first
        if static_keys::is_next_match(&key) {
            self.in_file_search_next();
            return;
        }
        if static_keys::is_prev_match(&key) {
            self.in_file_search_prev();
            return;
        }
        if let Some(toggle) = static_keys::search_toggle(&key) {
            if let Some(s) = &mut self.in_file_search {
                match toggle {
                    static_keys::SearchToggle::Regex => s.regex = !s.regex,
                    static_keys::SearchToggle::CaseSensitive => {
                        s.case_sensitive = !s.case_sensitive
                    }
                    static_keys::SearchToggle::WholeWord => s.whole_word = !s.whole_word,
                }
            }
            self.refresh_in_file_search();
            self.search_input_feedback();
            self.scroll_in_file_search_to_current();
            return;
        }
        let Some(ref mut s) = self.in_file_search else {
            return;
        };
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate | OverlayKey::Close => {
                self.in_file_search_open = false;
            }
            OverlayKey::Handled => {
                self.refresh_in_file_search();
                self.search_input_feedback();
                self.scroll_in_file_search_to_current();
            }
            _ => {}
        }
    }

    pub(super) fn handle_filter_bar_key(&mut self, key: KeyEvent) {
        let Some(ref mut s) = self.filter_bar else {
            return;
        };
        let cancel = key.code == KeyCode::Esc;
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate | OverlayKey::Close => {
                if cancel || s.query.is_empty() {
                    self.filter_query = None;
                } else {
                    self.filter_query = Some(s.query.clone());
                }
                self.filter_bar = None;
                self.rebuild_filter_display_map();
                self.clamp_content_scroll();
            }
            OverlayKey::Handled => {
                self.filter_query = Some(s.query.clone());
                self.rebuild_filter_display_map();
                self.clamp_content_scroll();
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

    /// Shows immediate feedback for an edited in-file query.
    pub(crate) fn search_input_feedback(&mut self) {
        let Some(s) = self.in_file_search.as_ref() else {
            return;
        };
        if s.query.is_empty() {
            return;
        }
        if s.matches.is_empty() {
            self.set_status("Search: no matches");
        } else {
            self.set_status(format!("Search: {} matches", s.matches.len()));
        }
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

    /// Handles keyboard input while the repo-wide commit log overlay is open.
    /// Enter enters compare mode for the selected commit; Down at the end of
    /// the list triggers paged loading.
    pub(super) fn handle_repo_log_key(&mut self, key: KeyEvent) {
        let should_load_more = if let Some(ref r) = self.repo_log {
            // 'j' navigates only while the query is empty; otherwise it is a
            // query character and must not trigger paging.
            (matches!(key.code, KeyCode::Down)
                || (matches!(key.code, KeyCode::Char('j')) && r.query.is_empty()))
                && r.selected + 1 >= r.results_len()
        } else {
            false
        };
        if should_load_more {
            if let Some(ref mut r) = self.repo_log {
                r.load_more();
            }
        }
        let Some(ref mut r) = self.repo_log else {
            return;
        };
        match handle_list_picker_key(r, &key) {
            OverlayKey::Activate => self.show_repo_log_compare(),
            OverlayKey::Close => self.repo_log = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the theme picker overlay is open.
    ///
    /// Navigation (j/k/arrows) previews the highlighted theme live behind the
    /// popup; Esc reverts to the original theme that was active before the
    /// picker opened; Enter commits the previewed theme to config.
    pub(super) fn handle_theme_key(&mut self, key: KeyEvent) {
        let (result, selected_name, selected_theme) = {
            let Some(ref mut p) = self.theme_picker else {
                return;
            };
            let result = handle_list_picker_key(p, &key);
            let selected_name = p.selected_name().map(String::from);
            // The picker already parsed every theme once in `discover_all`;
            // reuse that instead of re-reading/re-parsing from disk on
            // every navigation keystroke.
            let selected_theme = p.selected_theme().cloned();
            (result, selected_name, selected_theme)
        };
        match result {
            OverlayKey::Activate => {
                self.apply_selected_theme();
                self.context_menu = None;
            }
            OverlayKey::Close => {
                self.theme_picker = None;
                self.context_menu = None;
                // Revert to the original theme that was active before the picker
                // opened (self.config.theme is never written during preview).
                let config_theme = self.config.theme.clone();
                let theme = config_theme.resolve();
                let requested_name = config_theme
                    .name
                    .clone()
                    .unwrap_or_else(|| "default".to_string());
                // resolve() silently falls back to the default theme when the
                // configured name can't be loaded; report that same fallback
                // name here so the UI isn't told an invalid theme name.
                let name = if Theme::load(&requested_name).is_some() {
                    requested_name
                } else {
                    "default".to_string()
                };
                self.apply_theme(&name, theme);
            }
            OverlayKey::Handled => {
                if let (Some(name), Some(theme)) = (selected_name, selected_theme) {
                    self.apply_theme(&name, theme);
                }
            }
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

    /// Handles keyboard input while the bookmarks overlay is open.
    pub(super) fn handle_bookmarks_key(&mut self, key: KeyEvent) {
        let Some(ref mut picker) = self.bookmarks else {
            return;
        };
        match handle_list_picker_key(picker, &key) {
            OverlayKey::Activate => self.activate_bookmark_selection(),
            OverlayKey::Close => self.bookmarks = None,
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "overlay_test.rs"]
mod tests;
