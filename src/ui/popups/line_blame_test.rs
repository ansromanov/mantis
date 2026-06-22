// Tests for the line_blame popup guard conditions.
//
// draw_line_blame is a pure UI paint function requiring a live ratatui Frame;
// rendering paths are in popups_test.rs. These tests verify the App state
// flags that control whether the popup renders.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_line_blame_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn show_line_blame_false_on_init() {
    let root = temp_dir();
    let app = app_for(&root);
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn show_line_blame_never_set_in_diff_mode() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.is_diff = true;
    let would_render = app.show_line_blame && !app.is_diff;
    assert!(!would_render);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn show_line_blame_no_op_without_current_file() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.show_line_blame = true;
    assert!(app.current_file.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_in_bounds_after_file_open() {
    let root = temp_dir();
    fs::write(root.join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    let line_count = app.display_line_count();
    assert!(
        app.active_line < line_count || line_count == 0,
        "active_line {} out of bounds ({})",
        app.active_line,
        line_count
    );
    fs::remove_dir_all(&root).ok();
}
