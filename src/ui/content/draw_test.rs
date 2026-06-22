// Tests for active-line highlight logic in draw_content.
//
// Full rendering tests for draw_content are in content_test.rs.
// These tests cover the active-line highlight guard introduced by this PR.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{App, Focus};
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_draw_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let long: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn active_line_initialises_to_zero_on_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_saturates_at_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;

    for _ in 0..200 {
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    }
    let max = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_guard_skipped_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert!(app.is_diff);
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}
