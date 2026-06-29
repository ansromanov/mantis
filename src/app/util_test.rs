use std::fs;
use std::path::PathBuf;

use ratatui::layout::Rect;

use super::*;
use crate::app::App;
use crate::config::Config;

fn temp_tree() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_util_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn rect_contains_checks_bounds() {
    let r = Rect {
        x: 2,
        y: 3,
        width: 4,
        height: 5,
    };
    assert!(rect_contains(r, 2, 3)); // top-left corner
    assert!(rect_contains(r, 5, 7)); // inside, near far corner
    assert!(!rect_contains(r, 6, 3)); // column == x + width
    assert!(!rect_contains(r, 2, 8)); // row == y + height
    assert!(!rect_contains(r, 1, 3)); // left of area
    assert!(!rect_contains(r, 2, 2)); // above area
}

// -- diff_line_style ------------------------------------------------------

#[test]
fn diff_line_style_hunk_header_uses_accent() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("@@ -1,3 +1,4 @@", &app.theme);
    assert_eq!(style.fg, Some(app.theme.accent));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_file_header_uses_dim_bold() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("+++ b/file.rs", &app.theme);
    assert_eq!(style.fg, Some(app.theme.dim));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_addition_uses_diff_add() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("+new line", &app.theme);
    assert_eq!(style.fg, Some(app.theme.diff_add));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_removal_uses_diff_del() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("-old line", &app.theme);
    assert_eq!(style.fg, Some(app.theme.diff_del));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_diff_meta_uses_dim() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("diff --git a/file b/file", &app.theme);
    assert_eq!(style.fg, Some(app.theme.dim));
    let style2 = diff_line_style("index abc..def", &app.theme);
    assert_eq!(style2.fg, Some(app.theme.dim));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_plain_text_uses_default() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style(" context line", &app.theme);
    assert_eq!(style.fg, None);
    fs::remove_dir_all(&root).ok();
}

// -- deleted_set -----------------------------------------------------------

#[test]
fn deleted_set_returns_empty_when_disabled() {
    let map = std::collections::HashMap::new();
    let result = deleted_set(&map, false);
    assert!(result.is_empty());
}

#[test]
fn deleted_set_filters_existing_files() {
    let root = temp_tree();
    let mut map = std::collections::HashMap::new();
    map.insert(root.join("a.txt"), crate::git::GitStatus::Deleted);
    let result = deleted_set(&map, true);
    assert!(result.is_empty(), "existing files are not in deleted set");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn deleted_set_includes_absent_deleted_files() {
    let root = temp_tree();
    let gone = root.join("gone.txt");
    let mut map = std::collections::HashMap::new();
    map.insert(gone.clone(), crate::git::GitStatus::Deleted);
    let result = deleted_set(&map, true);
    assert!(result.contains(&gone));
    fs::remove_dir_all(&root).ok();
}
