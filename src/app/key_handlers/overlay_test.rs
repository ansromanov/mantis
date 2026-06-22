use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::GotoLineState;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_overlay_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn handle_goto_line_key_open_binding_not_appended_to_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.goto_line = Some(GotoLineState::new());
    // pressing the open binding ':' while dialog is open should not append to query
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.as_ref().unwrap().query.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_digit_appends_to_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.goto_line = Some(GotoLineState::new());
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::empty()));
    assert_eq!(app.goto_line.as_ref().unwrap().query, "5");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_esc_closes_dialog() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.goto_line = Some(GotoLineState::new());
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}
