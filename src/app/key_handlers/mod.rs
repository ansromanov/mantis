//! Keyboard dispatch entry point for `App`.
//!
//! `handle_key` is the single funnel for every key event. It first filters out
//! key-release events (Windows reports both press and release) so each action
//! fires once, then routes by precedence: modal overlays (about, help, theme
//! picker, history, recent files, command palette, search) consume input first,
//! and only when none is active does control fall through to the normal
//! tree/content handler. The actual handling lives in the sibling submodules
//! wired up here: `normal` (no overlay), `overlay` (search/picker editing),
//! and `editor` (command dispatch and
//! external-editor suspend/resume).
//!
//! Protocol 3+ key consumption: when at least one running plugin subscribes
//! to `on_keypress`, a normal-mode key is not handled immediately. Instead
//! `App::pending_keypress` is set with a `KEY_CONSUME_TIMEOUT` deadline, and
//! `App::process_pending_keypress` (called once per tick, see `app::refresh`)
//! decides whether to swallow it (a `key_handled` reply arrived in time) or
//! fall through to `handle_normal_key` (the deadline passed with no reply).
//! A new key arriving while one is already pending immediately falls through
//! the stale one via `App::preempt_pending_keypress` before being dispatched,
//! so no keystroke is dropped waiting on a plugin that will never see it.

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

use std::time::Duration;

use crossterm::event::KeyCode;

use super::{App, PendingKeypress};
use crate::config::static_keys;

/// Page size for help popup scrolling (matches `handle_list_picker_key`).
const HELP_PAGE_SIZE: usize = 10;

/// How long `App` waits for a `key_handled` reply before falling through to
/// normal-mode handling (protocol 3+ `on_keypress` key consumption). A bit
/// more than one ~16ms tick, so a plugin gets one full round trip. Longer
/// under `cfg(test)` so a real spawned subprocess's round trip (fork/exec +
/// pipe I/O) isn't racing a razor-thin window, matching the `REQUEST_TIMEOUT`
/// pattern in `crate::plugin::manager`.
#[cfg(not(test))]
const KEY_CONSUME_TIMEOUT: Duration = Duration::from_millis(20);
#[cfg(test)]
const KEY_CONSUME_TIMEOUT: Duration = Duration::from_secs(2);

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
        // Notify plugins of each keypress *only* in normal mode (no overlay
        // active) so search/picker input is not broadcast. Plugins receive
        // the key as a readable string: "q", "ctrl+c", "Enter", etc.
        if self.show_telemetry_notice {
            if static_keys::is_modal_close(&key) {
                self.show_telemetry_notice = false;
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
                        self.help_tab = (self.help_tab + 1) % crate::ui::popups::HELP_TABS.len();
                        self.help_scroll.scroll = 0;
                    }
                    KeyCode::Left | KeyCode::Char('h') | KeyCode::BackTab => {
                        self.help_tab = if self.help_tab == 0 {
                            crate::ui::popups::HELP_TABS.len() - 1
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
        } else if self.plugin_picker.is_some() {
            self.handle_plugin_key(key);
        } else if self.command_palette.is_some() {
            self.handle_command_key(key);
        } else if self.history.is_some() {
            self.handle_history_key(key);
        } else if self.recent_files.is_some() {
            self.handle_recent_key(key);
        } else if self.search.is_some() {
            self.handle_search_key(key);
        } else if self.in_file_search.is_some() {
            self.handle_in_file_search_key(key);
        } else if self.tree_filter.is_some() {
            self.handle_tree_filter_key(key);
        } else if self.bug_report.is_some() {
            self.handle_bug_report_key(key);
        } else if self.compare_input.is_some() {
            self.handle_compare_input_key(key);
        } else if self.goto_line.is_some() {
            self.handle_goto_line_key(key);
        } else {
            self.dispatch_normal_keypress(key);
        }
    }

    /// Dispatches a normal-mode keypress: notifies plugins, then either runs
    /// the built-in handler immediately (no plugin subscribes to
    /// `on_keypress`) or defers it for up to `KEY_CONSUME_TIMEOUT` to give a
    /// subscriber a chance to claim it via `key_handled` (protocol 3+). See
    /// the module doc for the full deferred-consumption flow.
    fn dispatch_normal_keypress(&mut self, key: crossterm::event::KeyEvent) {
        // A previous keypress may still be waiting on a `key_handled` reply
        // (e.g. rapid typing within one tick); resolve it now by falling
        // through, so it is never silently dropped.
        self.preempt_pending_keypress();
        self.plugin_manager.on_keypress(&key);
        if self.plugin_manager.has_keypress_subscriber() {
            // A stale `key_handled` reply for an already-resolved keypress
            // (e.g. one that fell through via deadline before the reply
            // arrived) must not be misread as claiming this new key.
            self.pending_keypress_handled = false;
            self.pending_keypress = Some(PendingKeypress {
                key,
                deadline: self.now() + KEY_CONSUME_TIMEOUT,
            });
        } else {
            self.handle_normal_key(key);
        }
    }
}
