use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::config::Config;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_mod_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn json_query_overlay_takes_key_precedence() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.json_query = Some(String::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::empty()));
    assert_eq!(app.json_query.as_deref(), Some("."));
    fs::remove_dir_all(root).ok();
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

#[test]
fn enter_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.show_about, "Enter must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_about, "Esc must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_closes_theme_picker_even_if_a_stale_context_menu_exists() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(crate::search::ThemePicker::default());
    app.context_menu = Some(crate::app::ContextMenuState {
        entries: Vec::new(),
        selected: 0,
        anchor: (0, 0),
        target: crate::app::ContextMenuTarget::Content,
        submenu_stack: Vec::new(),
    });

    app.handle_key(key(KeyCode::Esc));

    assert!(app.theme_picker.is_none());
    assert!(app.context_menu.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn menu_bar_consumes_escape_before_normal_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_menu_bar();
    app.handle_key(key(KeyCode::Esc));
    assert!(app.menu_bar_state.is_none());
    assert!(!app.should_quit);
    fs::remove_dir_all(root).ok();
}

#[test]
fn q_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.show_about, "q must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn o_does_not_close_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Char('o')));
    assert!(
        app.show_about,
        "o must NOT close the About dialog (it opens the release URL)"
    );
    fs::remove_dir_all(&root).ok();
}

// -- protocol 3: on_keypress key consumption -----------------------------------

// -- welcome overlay key handling --------------------------------------------

#[test]
fn esc_dismisses_welcome() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_welcome = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_welcome, "Esc must dismiss the welcome overlay");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn non_esc_keys_are_blocked_by_welcome() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);

    // Enable welcome, then press Down — it should NOT navigate tree.
    app.show_welcome = true;
    let before = app.tree_selected;
    app.handle_key(key(KeyCode::Down));
    assert_eq!(
        app.tree_selected, before,
        "non-Esc keys must be blocked (selected remained {before})"
    );

    // Welcome is still shown after non-Esc key.
    assert!(app.show_welcome, "welcome must survive non-Esc keys");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_dismisses_welcome_and_normal_keys_still_work() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    app.show_welcome = true;

    // Dismiss the welcome overlay.
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_welcome, "welcome dismissed");

    // Now pressing Down should move the tree selection.
    let before = app.tree_selected;
    app.handle_key(key(KeyCode::Down));
    assert!(
        app.tree_selected > before || app.tree_selected >= app.nodes.len() - 1,
        "after dismiss, Down must navigate tree (selected changed from {before} to {})",
        app.tree_selected
    );
    fs::remove_dir_all(&root).ok();
}
