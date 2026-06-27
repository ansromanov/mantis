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
