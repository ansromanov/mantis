use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn make_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

fn make_key_with_modifier(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

#[test]
fn test_is_close() {
    assert!(is_close(&make_key(KeyCode::Esc)));
    assert!(!is_close(&make_key(KeyCode::Enter)));
    assert!(!is_close(&make_key(KeyCode::Char('q'))));
}

#[test]
fn test_is_activate() {
    assert!(is_activate(&make_key(KeyCode::Enter)));
    assert!(!is_activate(&make_key(KeyCode::Esc)));
    assert!(!is_activate(&make_key(KeyCode::Char('o'))));
}

#[test]
fn test_is_page_up() {
    assert!(is_page_up(&make_key(KeyCode::PageUp)));
    assert!(!is_page_up(&make_key(KeyCode::Up)));
    assert!(!is_page_up(&make_key(KeyCode::PageDown)));
}

#[test]
fn test_is_page_down() {
    assert!(is_page_down(&make_key(KeyCode::PageDown)));
    assert!(!is_page_down(&make_key(KeyCode::Down)));
    assert!(!is_page_down(&make_key(KeyCode::PageUp)));
}

#[test]
fn test_is_prev_match() {
    assert!(is_prev_match(&make_key(KeyCode::Up)));
    assert!(is_prev_match(&make_key(KeyCode::BackTab)));
    assert!(is_prev_match(&make_key(KeyCode::Char('N'))));
    // Ctrl+P is the command palette, never search navigation.
    assert!(!is_prev_match(&make_key_with_modifier(
        KeyCode::Char('p'),
        KeyModifiers::CONTROL
    )));
    assert!(!is_prev_match(&make_key(KeyCode::Char('P')))); // P without Ctrl
    assert!(!is_prev_match(&make_key(KeyCode::Down)));
    assert!(!is_prev_match(&make_key(KeyCode::Char('n'))));
}

#[test]
fn test_is_next_match() {
    assert!(is_next_match(&make_key(KeyCode::Down)));
    assert!(is_next_match(&make_key(KeyCode::Char('n'))));
    assert!(is_next_match(&make_key(KeyCode::Tab)));
    assert!(!is_next_match(&make_key(KeyCode::Up)));
    assert!(!is_next_match(&make_key(KeyCode::BackTab)));
    assert!(!is_next_match(&make_key(KeyCode::Char('N'))));
}

#[test]
fn test_is_delete_char() {
    assert!(is_delete_char(&make_key(KeyCode::Backspace)));
    assert!(!is_delete_char(&make_key(KeyCode::Delete)));
    assert!(!is_delete_char(&make_key(KeyCode::Char('x'))));
}

#[test]
fn test_is_toggle_modal() {
    assert!(is_toggle_modal(&make_key(KeyCode::Tab)));
    assert!(!is_toggle_modal(&make_key(KeyCode::BackTab)));
    assert!(!is_toggle_modal(&make_key(KeyCode::Char(' '))));
}

#[test]
fn test_is_toggle_selection() {
    assert!(is_toggle_selection(&make_key(KeyCode::Char(' '))));
    assert!(!is_toggle_selection(&make_key(KeyCode::Tab)));
    assert!(!is_toggle_selection(&make_key(KeyCode::Char('x'))));
}

#[test]
fn test_is_open_release() {
    assert!(is_open_release(&make_key(KeyCode::Char('o'))));
    assert!(!is_open_release(&make_key(KeyCode::Char('O'))));
    assert!(!is_open_release(&make_key(KeyCode::Char('?'))));
}

#[test]
fn test_is_modal_close() {
    assert!(is_modal_close(&make_key(KeyCode::Char('?'))));
    assert!(is_modal_close(&make_key(KeyCode::Char('q'))));
    assert!(is_modal_close(&make_key(KeyCode::Esc)));
    assert!(is_modal_close(&make_key(KeyCode::Enter)));
    assert!(!is_modal_close(&make_key(KeyCode::Char('o'))));
    assert!(!is_modal_close(&make_key(KeyCode::Char('n'))));
}

#[test]
fn test_search_toggle_ctrl_bindings() {
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('r'),
            KeyModifiers::CONTROL
        )),
        Some(SearchToggle::Regex)
    );
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('R'),
            KeyModifiers::CONTROL | KeyModifiers::SHIFT
        )),
        Some(SearchToggle::Regex)
    );
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('a'),
            KeyModifiers::CONTROL
        )),
        Some(SearchToggle::CaseSensitive)
    );
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('w'),
            KeyModifiers::CONTROL
        )),
        Some(SearchToggle::WholeWord)
    );
}

#[test]
fn test_search_toggle_requires_ctrl() {
    // Bare letters must fall through to query input.
    assert_eq!(search_toggle(&make_key(KeyCode::Char('r'))), None);
    assert_eq!(search_toggle(&make_key(KeyCode::Char('a'))), None);
    assert_eq!(search_toggle(&make_key(KeyCode::Char('w'))), None);
    // Alt is banned for new bindings and must not trigger toggles.
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('r'),
            KeyModifiers::ALT
        )),
        None
    );
}

#[test]
fn test_search_toggle_ignores_other_ctrl_keys() {
    assert_eq!(
        search_toggle(&make_key_with_modifier(
            KeyCode::Char('p'),
            KeyModifiers::CONTROL
        )),
        None
    );
    assert_eq!(
        search_toggle(&make_key_with_modifier(KeyCode::Tab, KeyModifiers::CONTROL)),
        None
    );
}
