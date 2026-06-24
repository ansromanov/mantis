use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::*;

fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[test]
fn plain_char() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Char('q'), KeyModifiers::NONE)),
        "q"
    );
}

#[test]
fn ctrl_modifier() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Char('c'), KeyModifiers::CONTROL)),
        "ctrl+c"
    );
}

#[test]
fn alt_modifier() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Char('.'), KeyModifiers::ALT)),
        "alt+."
    );
}

#[test]
fn ctrl_alt_modifier() {
    assert_eq!(
        key_event_to_string(&key(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL | KeyModifiers::ALT
        )),
        "ctrl+alt+x"
    );
}

#[test]
fn named_keys() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Enter, KeyModifiers::NONE)),
        "Enter"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Tab, KeyModifiers::NONE)),
        "Tab"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Esc, KeyModifiers::NONE)),
        "Esc"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Backspace, KeyModifiers::NONE)),
        "Backspace"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Up, KeyModifiers::NONE)),
        "Up"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Down, KeyModifiers::NONE)),
        "Down"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Left, KeyModifiers::NONE)),
        "Left"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Right, KeyModifiers::NONE)),
        "Right"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::PageUp, KeyModifiers::NONE)),
        "PageUp"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::PageDown, KeyModifiers::NONE)),
        "PageDown"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::Home, KeyModifiers::NONE)),
        "Home"
    );
    assert_eq!(
        key_event_to_string(&key(KeyCode::End, KeyModifiers::NONE)),
        "End"
    );
}

#[test]
fn space_char() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Char(' '), KeyModifiers::NONE)),
        "Space"
    );
}

#[test]
fn ctrl_enter() {
    assert_eq!(
        key_event_to_string(&key(KeyCode::Enter, KeyModifiers::CONTROL)),
        "ctrl+Enter"
    );
}
