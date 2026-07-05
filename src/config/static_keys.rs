//! Reserved (non-configurable) modal keys.
//!
//! Defines intent-based predicates for keys that are reserved and not user-rewritable:
//! Esc, Enter, Up/Down, PageUp/PageDown, Backspace, Tab/BackTab, and printable Char.
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

/// In-file search navigation: previous match (Up, N, BackTab, Ctrl+P).
pub fn is_prev_match(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Up | KeyCode::BackTab => true,
        KeyCode::Char('N') => true,
        KeyCode::Char('P') => key.modifiers.intersects(KeyModifiers::CONTROL),
        _ => false,
    }
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

/// Toggle regex search (Alt+R on PC, Cmd+Alt+R or Ctrl+Alt+R on macOS).
pub fn is_toggle_regex(key: &KeyEvent) -> bool {
    let matches_char = matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'));
    if !matches_char {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        let has_super = key.modifiers.contains(KeyModifiers::SUPER);
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let has_alt = key.modifiers.contains(KeyModifiers::ALT);
        (has_super || has_ctrl) && has_alt
    }
    #[cfg(not(target_os = "macos"))]
    {
        key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL)
    }
}

/// Toggle case-sensitive search (Alt+C on PC, Cmd+Alt+C or Ctrl+Alt+C on macOS).
pub fn is_toggle_case(key: &KeyEvent) -> bool {
    let matches_char = matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C'));
    if !matches_char {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        let has_super = key.modifiers.contains(KeyModifiers::SUPER);
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let has_alt = key.modifiers.contains(KeyModifiers::ALT);
        (has_super || has_ctrl) && has_alt
    }
    #[cfg(not(target_os = "macos"))]
    {
        key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL)
    }
}

/// Toggle whole-word search (Alt+W on PC, Cmd+Alt+W or Ctrl+Alt+W on macOS).
pub fn is_toggle_whole_word(key: &KeyEvent) -> bool {
    let matches_char = matches!(key.code, KeyCode::Char('w') | KeyCode::Char('W'));
    if !matches_char {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        let has_super = key.modifiers.contains(KeyModifiers::SUPER);
        let has_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let has_alt = key.modifiers.contains(KeyModifiers::ALT);
        (has_super || has_ctrl) && has_alt
    }
    #[cfg(not(target_os = "macos"))]
    {
        key.modifiers.contains(KeyModifiers::ALT) && !key.modifiers.contains(KeyModifiers::CONTROL)
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
