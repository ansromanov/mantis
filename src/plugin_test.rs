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
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let old = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: ENV_LOCK serialises all callers; no other thread mutates this var.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", "/tmp/custom_cfg") };
    let dir = default_plugin_dir();
    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }
    assert!(dir.starts_with("/tmp/custom_cfg/tree-viewer/plugins"));
}

#[test]
#[cfg(not(windows))]
fn install_bundled_plugins_creates_scripts() {
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = std::env::temp_dir().join(format!("tv_plugin_test_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let old = std::env::var_os("XDG_CONFIG_HOME");
    // SAFETY: ENV_LOCK serialises all callers; no other thread mutates this var.
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &tmp) };

    install_bundled_plugins();

    unsafe {
        match old {
            Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
    }

    let plugins_dir = tmp.join("tree-viewer").join("plugins");
    assert!(plugins_dir.is_dir(), "plugins directory should be created");
    assert!(
        plugins_dir.join("git-diff.sh").exists(),
        "git-diff.sh must be installed"
    );
    assert!(
        plugins_dir.join("git-log.sh").exists(),
        "git-log.sh must be installed"
    );
    assert!(
        plugins_dir.join("syntaxes").is_dir(),
        "syntaxes subdirectory should be created"
    );
    assert!(
        plugins_dir
            .join("syntaxes")
            .join("terraform.sublime-syntax")
            .exists(),
        "terraform.sublime-syntax must be installed"
    );
    std::fs::remove_dir_all(&tmp).ok();
}

// -- PluginManager lifecycle --------------------------------------------------

#[test]
fn plugin_entries_empty_when_no_plugins_registered() {
    let mgr = PluginManager::new(vec![]);
    assert!(mgr.plugin_entries().is_empty());
}

#[test]
fn plugin_entries_shows_registered_plugins_as_not_running() {
    let entry = PluginEntry {
        path: std::path::PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mgr = PluginManager::new(vec![("test-plugin".to_string(), entry)]);
    let entries = mgr.plugin_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].0, "test-plugin");
    assert!(!entries[0].1, "unstarted plugin must not show as running");
}

#[test]
fn activate_one_errors_on_unknown_name() {
    let mut mgr = PluginManager::new(vec![]);
    assert!(mgr.activate_one("ghost", None).is_err());
}

#[test]
fn activate_one_errors_on_bad_path() {
    let entry = PluginEntry {
        path: std::path::PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("bad".to_string(), entry)]);
    assert!(mgr.activate_one("bad", None).is_err());
}

#[test]
fn deactivate_one_is_noop_when_plugin_not_running() {
    let entry = PluginEntry {
        path: std::path::PathBuf::from("/nonexistent/plugin"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("p".to_string(), entry)]);
    mgr.deactivate_one("p"); // must not panic
    assert!(!mgr.plugin_entries()[0].1);
}

#[test]
#[cfg(unix)]
fn activate_one_then_deactivate_one_updates_running_state() {
    // /bin/cat acts as a stub plugin: blocks on stdin reads, never writes.
    let entry = PluginEntry {
        path: std::path::PathBuf::from("/bin/cat"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("cat-stub".to_string(), entry)]);

    assert!(
        !mgr.plugin_entries()[0].1,
        "should not be running before activate"
    );
    mgr.activate_one("cat-stub", None).expect("spawn /bin/cat");
    assert!(
        mgr.plugin_entries()[0].1,
        "should be running after activate"
    );
    mgr.deactivate_one("cat-stub");
    assert!(
        !mgr.plugin_entries()[0].1,
        "should not be running after deactivate"
    );
}

#[test]
#[cfg(unix)]
fn activate_one_is_noop_when_already_running() {
    let entry = PluginEntry {
        path: std::path::PathBuf::from("/bin/cat"),
        enabled: false,
        ..Default::default()
    };
    let mut mgr = PluginManager::new(vec![("cat-stub".to_string(), entry)]);
    mgr.activate_one("cat-stub", None).expect("first spawn");
    mgr.activate_one("cat-stub", None)
        .expect("second call must be noop");
    assert_eq!(
        mgr.plugin_entries().iter().filter(|(_, r)| *r).count(),
        1,
        "must still be only one running instance"
    );
    mgr.deactivate_all();
}
