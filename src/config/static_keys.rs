//! Reserved (non-configurable) modal keys.
//!
//! Defines intent-based predicates for keys that are reserved and not user-rewritable:
//! Esc, Enter, Up/Down, PageUp/PageDown, Backspace, Tab/BackTab, printable Char,
//! and the search-option toggles (Ctrl+R / Ctrl+A / Ctrl+W).
//! This is the single source of truth for modal keybindings; all overlays and modals
//! should use these predicates instead of matching `KeyCode::*` directly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Modal-only. Closes/cancels the active overlay (Esc).
pub fn is_close(key: &KeyEvent) -> bool {
    key.code == KeyCode::Esc
}

/// Modal-only. Activates the selected item (Enter).
#[allow(dead_code)]
pub fn is_activate(key: &KeyEvent) -> bool {
    key.code == KeyCode::Enter
}

/// Modal list pagination: page up (PageUp).
pub fn is_page_up(key: &KeyEvent) -> bool {
    key.code == KeyCode::PageUp
}

/// Modal list pagination: page down (PageDown).
pub fn is_page_down(key: &KeyEvent) -> bool {
    key.code == KeyCode::PageDown
}

/// In-file search navigation: previous match (Up, N, BackTab).
/// Deliberately no Ctrl+P: that is the command palette, which must stay
/// reachable while the search bar is open.
pub fn is_prev_match(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Up | KeyCode::BackTab | KeyCode::Char('N')
    )
}

/// In-file search navigation: next match (Down, n, Tab).
pub fn is_next_match(key: &KeyEvent) -> bool {
    matches!(key.code, KeyCode::Down | KeyCode::Char('n') | KeyCode::Tab)
}

/// Modal-only. Deletes character or closes on empty (Backspace).
#[allow(dead_code)]
pub fn is_delete_char(key: &KeyEvent) -> bool {
    key.code == KeyCode::Backspace
}

/// Modal overlay toggle (Tab for search: file/content mode toggle).
pub fn is_toggle_modal(key: &KeyEvent) -> bool {
    key.code == KeyCode::Tab
}

/// Search-option toggles available inside the search overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchToggle {
    /// Regular-expression matching (Ctrl+R).
    Regex,
    /// Case-sensitive matching (Ctrl+A, mirroring the `[Aa]` indicator).
    CaseSensitive,
    /// Whole-word matching (Ctrl+W).
    WholeWord,
}

/// Maps a key event to the search toggle it activates, if any.
/// Ctrl-only bindings — the Alt modifier is unreliable across terminals
/// and banned for new bindings.
pub fn search_toggle(key: &KeyEvent) -> Option<SearchToggle> {
    if !key.modifiers.contains(KeyModifiers::CONTROL) {
        return None;
    }
    match key.code {
        KeyCode::Char('r') | KeyCode::Char('R') => Some(SearchToggle::Regex),
        KeyCode::Char('a') | KeyCode::Char('A') => Some(SearchToggle::CaseSensitive),
        KeyCode::Char('w') | KeyCode::Char('W') => Some(SearchToggle::WholeWord),
        _ => None,
    }
}

/// Plugin picker toggle (Space).
pub fn is_toggle_selection(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char(' ')
}

/// Modal about screen: open release URL (o).
pub fn is_open_release(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('o')
}

/// Modal about/help screen: close (?, q, Esc, Enter).
pub fn is_modal_close(key: &KeyEvent) -> bool {
    matches!(
        key.code,
        KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Esc | KeyCode::Enter
    )
}

#[cfg(test)]
#[path = "static_keys_test.rs"]
mod tests;
