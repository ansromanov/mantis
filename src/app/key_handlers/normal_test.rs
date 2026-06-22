use crate::app::{App, Focus};
use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_normal_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn goto_line_keybinding_opens_dialog_with_content_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_keybinding_is_noop_with_tree_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}
