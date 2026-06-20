use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{default_plugin_dir, key_event_to_string};

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

// default_plugin_dir tests — we only verify the path ends with the expected suffix.

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_ends_with_suffix() {
    let dir = default_plugin_dir();
    let components: Vec<_> = dir.components().collect();
    let last_two: Vec<_> = components.iter().rev().take(2).collect();
    // .../<platform-config>/tree-viewer/plugins
    assert_eq!(
        last_two[0],
        &std::path::Component::Normal("plugins".as_ref())
    );
    assert_eq!(
        last_two[1],
        &std::path::Component::Normal("tree-viewer".as_ref())
    );
}

#[test]
#[cfg(not(windows))]
fn default_plugin_dir_respects_xdg() {
    // Temporarily override XDG_CONFIG_HOME
    std::env::set_var("XDG_CONFIG_HOME", "/tmp/custom_cfg");
    let dir = default_plugin_dir();
    std::env::remove_var("XDG_CONFIG_HOME");
    assert!(dir.starts_with("/tmp/custom_cfg/tree-viewer/plugins"));
}
