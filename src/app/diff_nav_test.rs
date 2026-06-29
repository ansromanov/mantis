use std::fs;
use std::path::PathBuf;

use super::*;
use crate::config::Config;
use crate::diff::{Cell, CellKind, DiffRow};
use ratatui::layout::Rect;

fn temp_tree() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_diff_nav_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("b.txt"), "hello\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn set_unified_diff(app: &mut App) {
    app.is_diff = true;
    app.diff_side_by_side = false;
    app.current_file = None;
    app.virtual_file = None;
    app.content = vec![
        "diff --git a/file.rs b/file.rs".to_string(),
        "index abc..def".to_string(),
        "--- a/file.rs".to_string(),
        "+++ b/file.rs".to_string(),
        "@@ -1,3 +1,4 @@".to_string(),
        " line1".to_string(),
        "-old line".to_string(),
        "+new line".to_string(),
        " line2".to_string(),
        "@@ -10,4 +11,5 @@".to_string(),
        " context".to_string(),
        "-gone".to_string(),
        "+added".to_string(),
        " still".to_string(),
        "@@ -20,3 +21,2 @@".to_string(),
        "-removed".to_string(),
        " kept".to_string(),
    ];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
}

#[test]
fn diff_next_hunk_moves_to_next_at_marker() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_unified_diff(&mut app);
    app.content_scroll = 0;

    app.diff_next_hunk();
    // Past scroll=0, first @@ is at index 4
    assert_eq!(app.content_scroll, 4);

    app.diff_next_hunk();
    // Next @@ is at index 9
    assert_eq!(app.content_scroll, 9);

    app.diff_next_hunk();
    // Next @@ is at index 14
    assert_eq!(app.content_scroll, 14);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_next_hunk_noop_when_at_last_hunk() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_unified_diff(&mut app);
    // Start past the last hunk
    app.content_scroll = 15;
    let before = app.content_scroll;
    app.diff_next_hunk();
    assert_eq!(app.content_scroll, before);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_prev_hunk_moves_to_previous_at_marker() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_unified_diff(&mut app);
    // Start past all hunks
    app.content_scroll = 20;

    app.diff_prev_hunk();
    // Going back from 20, last @@ is at 14
    assert_eq!(app.content_scroll, 14);

    app.diff_prev_hunk();
    // Previous @@ is at 9
    assert_eq!(app.content_scroll, 9);

    app.diff_prev_hunk();
    // Previous @@ is at 4
    assert_eq!(app.content_scroll, 4);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_prev_hunk_noop_when_at_first_hunk() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_unified_diff(&mut app);
    // At or above the first hunk
    app.content_scroll = 4;
    app.diff_prev_hunk();
    assert_eq!(app.content_scroll, 4);

    app.content_scroll = 0;
    app.diff_prev_hunk();
    assert_eq!(app.content_scroll, 0);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_next_hunk_noop_when_no_hunks() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    app.content = vec![
        "diff --git a/file.rs b/file.rs".to_string(),
        "index abc..def".to_string(),
    ];
    app.content_scroll = 0;
    let before = app.content_scroll;
    app.diff_next_hunk();
    assert_eq!(app.content_scroll, before);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_prev_hunk_noop_when_no_hunks() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    app.content = vec![
        "diff --git a/file.rs b/file.rs".to_string(),
        "index abc..def".to_string(),
    ];
    app.content_scroll = 5;
    let before = app.content_scroll;
    app.diff_prev_hunk();
    assert_eq!(app.content_scroll, before);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_next_hunk_uses_diff_rows_in_sbs_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    app.diff_side_by_side = true;
    app.current_file = None;
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 1,
    };
    app.diff_rows = vec![
        DiffRow::Header("@@ -1,3 +1,4 @@".to_string()),
        DiffRow::Split {
            left: Cell {
                kind: CellKind::Context,
                line_no: Some(1),
                text: " line1".to_string(),
                emphasis: Vec::new(),
            },
            right: Cell {
                kind: CellKind::Context,
                line_no: Some(1),
                text: " line1".to_string(),
                emphasis: Vec::new(),
            },
        },
        DiffRow::Header("@@ -10,4 +11,5 @@".to_string()),
        DiffRow::Split {
            left: Cell {
                kind: CellKind::Context,
                line_no: Some(10),
                text: " context".to_string(),
                emphasis: Vec::new(),
            },
            right: Cell {
                kind: CellKind::Context,
                line_no: Some(11),
                text: " context".to_string(),
                emphasis: Vec::new(),
            },
        },
    ];

    assert!(app.diff_sbs_active());

    app.content_scroll = 0;
    app.diff_next_hunk();
    // First header at index 0, but scroll >= 0 means "past this hunk",
    // so next is at index 2.
    assert_eq!(app.content_scroll, 2);

    app.diff_next_hunk();
    // No more headers after index 2
    assert_eq!(app.content_scroll, 2);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_prev_hunk_uses_diff_rows_in_sbs_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    app.diff_side_by_side = true;
    app.current_file = None;
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 1,
    };
    app.diff_rows = vec![
        DiffRow::Header("@@ -1,3 +1,4 @@".to_string()),
        DiffRow::Split {
            left: Cell {
                kind: CellKind::Context,
                line_no: Some(1),
                text: " line1".to_string(),
                emphasis: Vec::new(),
            },
            right: Cell {
                kind: CellKind::Context,
                line_no: Some(1),
                text: " line1".to_string(),
                emphasis: Vec::new(),
            },
        },
        DiffRow::Header("@@ -10,4 +11,5 @@".to_string()),
        DiffRow::Split {
            left: Cell {
                kind: CellKind::Context,
                line_no: Some(10),
                text: " context".to_string(),
                emphasis: Vec::new(),
            },
            right: Cell {
                kind: CellKind::Context,
                line_no: Some(11),
                text: " context".to_string(),
                emphasis: Vec::new(),
            },
        },
    ];

    assert!(app.diff_sbs_active());

    app.content_scroll = 3;
    app.diff_prev_hunk();
    // Previous header before index 3 is at index 2
    assert_eq!(app.content_scroll, 2);

    app.diff_prev_hunk();
    // Previous header before index 2 is at index 0
    assert_eq!(app.content_scroll, 0);

    app.diff_prev_hunk();
    // No more headers before index 0
    assert_eq!(app.content_scroll, 0);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_nav_noop_when_not_a_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = false;
    app.content_scroll = 5;
    let before = app.content_scroll;

    app.diff_next_hunk();
    assert_eq!(app.content_scroll, before);

    app.diff_prev_hunk();
    assert_eq!(app.content_scroll, before);

    fs::remove_dir_all(&root).ok();
}
