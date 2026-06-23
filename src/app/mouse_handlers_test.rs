use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{MouseEvent, MouseEventKind};
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

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
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
    app.tree_independent_scroll = true;
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
