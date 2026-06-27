use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_mouse_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let long: String = (1..=200).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn tree_with_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_mouse_dir_test_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    fs::write(dir.join("b.txt"), "world\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn left_down_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn scroll_down_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

#[test]
fn scrolling_content_marks_session_dirty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    // Give the content pane a real area so the scroll has somewhere to land and
    // content_scroll_max() is non-zero.
    app.content_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 10,
    };
    app.session_dirty = false;
    app.handle_mouse(scroll_down_at(50, 5));
    assert!(
        app.content_scroll > 0,
        "precondition: scrolling inside the content area must move content_scroll"
    );
    assert!(
        app.session_dirty,
        "scrolling content with the mouse must mark the session dirty so scroll position persists"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_outside_content_does_not_mark_dirty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 10,
    };
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.session_dirty = false;
    // Scroll over the tree pane: content_scroll is untouched, so no session write.
    app.handle_mouse(scroll_down_at(5, 5));
    assert_eq!(app.content_scroll, 0, "content scroll must be unchanged");
    assert!(
        !app.session_dirty,
        "scrolling outside the content pane must not mark the session dirty"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn wheel_over_tree_scrolls_without_moving_cursor() {
    let root = temp_tree();
    // Create enough files so the tree overflows a 1-row viewport.
    for i in 0..10 {
        fs::write(root.join(format!("extra{i}.txt")), "").unwrap();
    }
    let mut app = app_for(&root);
    assert!(!app.tree_independent_scroll); // default is false
    assert!(
        app.nodes.len() > 2,
        "temp_tree must have >2 nodes for this test; got {}",
        app.nodes.len()
    );
    app.content_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 10,
    };
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 1,
    };
    let selected_before = app.tree_selected;
    let scroll_before = app.tree_scroll;
    let max_scroll = app.tree_scroll_max();
    assert!(
        max_scroll > 0,
        "precondition: tree_scroll_max() must be > 0 (nodes={}, height={})",
        app.nodes.len(),
        app.tree_area.height
    );

    app.handle_mouse(scroll_down_at(5, 0));

    assert_eq!(
        app.tree_selected, selected_before,
        "mouse wheel must not move the selection"
    );
    assert!(
        app.tree_scroll > scroll_before,
        "mouse wheel must scroll the tree viewport"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn double_click_dir_descends_root() {
    let root = tree_with_dir();
    let mut app = app_for(&root);

    // Set up tree area so clicks land on the tree panel.
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };

    // First node is sub/ (dirs first, sorted by name).
    let sub_path = root.join("sub");
    assert!(
        app.nodes
            .first()
            .is_some_and(|n| n.is_dir && n.path == sub_path),
        "first node should be the sub/ directory"
    );

    // First click: single-click behavior — expands the directory.
    app.handle_mouse(left_down_at(5, 0));
    assert!(
        app.expanded.contains(&sub_path),
        "first click should expand the directory"
    );
    assert!(
        app.last_click.is_some(),
        "last_click should be set after first click"
    );

    // Second click (same row within 400ms): double-click — descend root.
    app.handle_mouse(left_down_at(5, 0));
    assert_eq!(
        app.root, sub_path,
        "double-click should change root to the clicked directory"
    );
    assert!(
        app.expanded.is_empty(),
        "expanded should be cleared after root change"
    );
    assert!(
        app.last_click.is_none(),
        "last_click cleared after a successful double-click"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn double_click_file_does_not_descend() {
    let root = tree_with_dir();
    let mut app = app_for(&root);

    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };

    // a.txt is at index 1 (after sub/).
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .expect("a.txt should exist");
    let orig_root = root.clone();

    // First click on a.txt.
    app.handle_mouse(left_down_at(5, file_idx as u16));

    // Second click on a.txt (simulated double-click).
    app.handle_mouse(left_down_at(5, file_idx as u16));

    // Root should NOT change for a file.
    assert_eq!(
        app.root, orig_root,
        "double-click on a file must not change root"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_single_click_does_not_navigate() {
    let root = tree_with_dir();
    let mut app = app_for(&root);

    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 4,
            height: 1,
        },
    ));

    let prev = app.tree_selected;
    app.handle_mouse(left_down_at(2, 1));

    assert_eq!(
        app.tree_selected, prev,
        "single click on breadcrumb must not navigate"
    );
    assert!(
        app.last_breadcrumb_click.is_some(),
        "single click must store pending click"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_double_click_navigates() {
    let root = tree_with_dir();
    let mut app = app_for(&root);

    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 4,
            height: 1,
        },
    ));

    // Prime the first click manually so the second is within 400 ms.
    app.last_breadcrumb_click = Some((Instant::now(), root.clone()));
    app.handle_mouse(left_down_at(2, 1));

    assert_eq!(
        app.tree_selected, 0,
        "double-click on root breadcrumb must select index 0"
    );
    assert!(
        app.last_breadcrumb_click.is_none(),
        "last_breadcrumb_click must be cleared after double-click"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_double_click_expired_does_not_navigate() {
    let root = tree_with_dir();
    let mut app = app_for(&root);

    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 4,
            height: 1,
        },
    ));

    let prev = app.tree_selected;
    // Stale first click (600 ms ago — past the 400 ms window).
    app.last_breadcrumb_click = Some((Instant::now() - Duration::from_millis(600), root.clone()));
    app.handle_mouse(left_down_at(2, 1));

    assert_eq!(
        app.tree_selected, prev,
        "expired double-click must not navigate"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn blame_column_click_opens_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 10,
        y: 2,
        width: 60,
        height: 20,
    };
    app.blame_col_width = 37;
    app.show_blame = true;
    app.show_line_blame = false;
    // rel_col = 11 - 10 = 1, inside blame_col_width (37).
    app.handle_mouse(left_down_at(11, 4));
    assert!(
        app.show_line_blame,
        "click inside the blame column must open the line-blame popup"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn click_past_blame_column_does_not_open_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 10,
        y: 2,
        width: 60,
        height: 20,
    };
    app.blame_col_width = 37;
    app.show_blame = true;
    app.show_line_blame = false;
    // rel_col = 52 - 10 = 42, past blame_col_width (37).
    app.handle_mouse(left_down_at(52, 4));
    assert!(
        !app.show_line_blame,
        "click past the blame column must not open the line-blame popup"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_click_with_no_filter_selects_node_directly() {
    let root = tree_with_dir();
    let mut app = app_for(&root);
    // tree_visible_indices is None (no filter) by default.
    assert!(app.tree_visible_indices.is_none());
    app.tree_area = Rect {
        x: 0,
        y: 2,
        width: 20,
        height: 20,
    };
    app.tree_offset = 0;
    // Click on the second row (index 1).
    app.handle_mouse(left_down_at(1, 3));
    assert_eq!(
        app.tree_selected, 1,
        "click on row 1 with no filter must select node index 1 directly"
    );
    fs::remove_dir_all(&root).ok();
}

// -- set_scroll_from_mouse_y tests --
// Note: app_for() auto-opens long.txt (200 lines) via open_selected_sync(),
// so display_line_count() = 200 in all three tests below.

#[test]
fn scrollbar_drag_no_op_when_content_fits_viewport() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Viewport taller than the 200-line file → total (200) <= inner_h (201), early return.
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 201,
    };
    app.content_scroll = 0;
    app.set_scroll_from_mouse_y(10);
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scrollbar_drag_maps_top_row_to_zero_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // 200 lines, 10-row viewport. Row 0 (top of track) must yield scroll 0.
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.set_scroll_from_mouse_y(0);
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scrollbar_drag_clamps_at_scroll_max() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // 200 lines, 10-row viewport → scroll_max = 190. A row past the end must stay ≤ max.
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.set_scroll_from_mouse_y(255);
    assert!(app.content_scroll <= app.content_scroll_max());
    fs::remove_dir_all(&root).ok();
}
