//! Shared trait and dispatcher for list-picker overlays.
//!
//! Defines `ListPicker` trait and `OverlayKey` enum plus `handle_list_picker_key`
//! function that provides uniform keyboard handling for all list-style overlays
//! (search, history, theme, recent files, commands, plugin, goto-line, in-file
//! search, and tree filter). Each overlay's handler becomes a thin wrapper:
//! handle extra keys first, fall through to the shared dispatcher, then map
//! `Activate`/`Close` to the overlay-specific action.

use crossterm::event::{KeyCode, KeyEvent};

/// Outcome of a key handled by the shared dispatcher.
#[derive(Debug, PartialEq, Eq)]
pub enum OverlayKey {
    /// The Enter key was pressed — caller should activate the selected item.
    Activate,
    /// The overlay should be dismissed (Esc or empty-backspace).
    Close,
    /// The key was consumed by navigation or query editing.
    Handled,
    /// The key was not recognised; caller may try other handling.
    Pass,
}

/// A list-style overlay with a text query and a selected row.
///
/// Implementations wrap the concrete type's query/selection fields. Types
/// without a query (e.g. `PluginPicker`) return `true` from `query_is_empty`
/// and no-op from `push`/`pop`.
pub trait ListPicker {
    fn query_push(&mut self, c: char);
    fn query_pop(&mut self);
    fn query_is_empty(&self) -> bool;
    fn results_len(&self) -> usize;
    fn selected(&self) -> usize;
    fn set_selected(&mut self, i: usize);
}

/// Shared key handling for any `ListPicker`.
///
/// Handles: Esc → Close, Enter → Activate, Up/Down/PageUp/PageDown → navigation,
/// Backspace → pop or Close if empty query, Char → push.
/// Returns what the caller should do with the result.
pub fn handle_list_picker_key<P: ListPicker>(p: &mut P, key: &KeyEvent) -> OverlayKey {
    match key.code {
        KeyCode::Esc => OverlayKey::Close,
        KeyCode::Enter => OverlayKey::Activate,
        KeyCode::Up => {
            p.set_selected(p.selected().saturating_sub(1));
            OverlayKey::Handled
        }
        // j/k navigate only when no query is active (vim-style); otherwise fall through to Char(c).
        KeyCode::Char('k') if p.query_is_empty() => {
            p.set_selected(p.selected().saturating_sub(1));
            OverlayKey::Handled
        }
        KeyCode::Down => {
            if p.selected() + 1 < p.results_len() {
                p.set_selected(p.selected() + 1);
            }
            OverlayKey::Handled
        }
        KeyCode::Char('j') if p.query_is_empty() => {
            if p.selected() + 1 < p.results_len() {
                p.set_selected(p.selected() + 1);
            }
            OverlayKey::Handled
        }
        KeyCode::PageUp => {
            p.set_selected(p.selected().saturating_sub(10));
            OverlayKey::Handled
        }
        KeyCode::PageDown => {
            let next = (p.selected() + 10).min(p.results_len().saturating_sub(1));
            p.set_selected(next);
            OverlayKey::Handled
        }
        KeyCode::Backspace => {
            if p.query_is_empty() {
                OverlayKey::Close
            } else {
                p.query_pop();
                OverlayKey::Handled
            }
        }
        KeyCode::Char(c) => {
            p.query_push(c);
            OverlayKey::Handled
        }
        _ => OverlayKey::Pass,
    }
}

#[cfg(test)]
#[path = "list_picker_test.rs"]
mod tests;
