// Tests for plugin-content geometry: selection extraction over styled spans and
// the line-number gutter being suppressed for plugin-rendered content.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::app::App;
use crate::config::Config;
use crate::selection::TextSelection;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_cpos_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("doc.md"), "placeholder\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn span(text: &str) -> Vec<(Style, String)> {
    vec![(Style::default(), text.to_string())]
}

/// Seed `plugin_content` (styled spans) and `plugin_content_text` for `path`.
fn seed_plugin(app: &mut App, path: PathBuf, lines: &[&str]) {
    let rendered: Vec<Vec<(Style, String)>> = lines.iter().map(|l| span(l)).collect();
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    app.plugin_content_text.insert(path.clone(), text);
    app.plugin_content.insert(path.clone(), rendered);
    app.current_file = Some(path);
}

#[test]
fn selection_text_single_line_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["hello world", "second line"]);
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 5),
    });
    assert_eq!(app.selection_text(), "hello");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_multi_line_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["hello world", "second line"]);
    // From col 6 of line 0 through col 6 of line 1.
    app.selection = Some(TextSelection {
        anchor: (0, 6),
        active: (1, 6),
    });
    assert_eq!(app.selection_text(), "world\nsecond");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_plugin_clamps_out_of_range_end() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["abc"]);
    // end_line past the buffer must clamp to the last line without panicking.
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (9, 99),
    });
    assert_eq!(app.selection_text(), "abc");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_prefix_width_zero_for_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["x", "y"]);
    app.show_line_numbers = true;
    assert_eq!(app.line_prefix_width(), 0);
    fs::remove_dir_all(&root).ok();
}

// -- content_scroll_max / set_content_scroll / clamp_content_scroll tests --

#[test]
fn set_content_scroll_clamps_to_max() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    // Simulate a content area 10 rows tall with 20 lines of content.
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..20).map(|i| format!("line {i}")).collect();
    assert_eq!(app.content_scroll_max(), 20usize.saturating_sub(10)); // 10

    app.set_content_scroll(usize::MAX);
    assert_eq!(app.content_scroll, app.content_scroll_max());

    app.set_content_scroll(5);
    assert_eq!(app.content_scroll, 5);

    app.set_content_scroll(app.content_scroll_max() + 100);
    assert_eq!(app.content_scroll, app.content_scroll_max());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_content_scroll_zero_always_works() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..5).map(|i| format!("line {i}")).collect();
    app.content_scroll = 3;
    app.set_content_scroll(0);
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn clamp_content_scroll_reduces_when_content_shrinks() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None; // clear auto-loaded vf from temp_root doc.md
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 5,
    };
    app.content = (0..100).map(|i| format!("line {i}")).collect();
    app.content_scroll = 90;
    // max = 100 - 5 = 95; 90 <= 95, so no change.
    app.clamp_content_scroll();
    assert_eq!(app.content_scroll, 90);

    // Content shrinks to 10 lines → max = 10 - 5 = 5.
    app.content = (0..10).map(|i| format!("line {i}")).collect();
    app.clamp_content_scroll();
    assert_eq!(app.content_scroll, 5);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn clamp_content_scroll_does_not_increase_when_content_grows() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None; // clear auto-loaded vf from temp_root doc.md
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 5,
    };
    app.content = (0..10).map(|i| format!("line {i}")).collect();
    app.content_scroll = 4;
    // Content grows → max increases, clamp leaves scroll at 4.
    app.content = (0..100).map(|i| format!("line {i}")).collect();
    app.clamp_content_scroll();
    assert_eq!(app.content_scroll, 4);
    fs::remove_dir_all(&root).ok();
}

// -- page_rows tests --

#[test]
fn page_rows_defaults_to_viewport_height_minus_one() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 25,
    };
    assert_eq!(app.page_rows(), 24);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn page_rows_at_least_one() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 0,
    };
    assert_eq!(app.page_rows(), 1);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    assert_eq!(app.page_rows(), 1);
    fs::remove_dir_all(&root).ok();
}

// -- has_text_cursor tests --

#[test]
fn has_text_cursor_true_for_normal_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = false;
    app.current_file = None;
    assert!(app.has_text_cursor());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn has_text_cursor_false_for_diff() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert!(!app.has_text_cursor());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn has_text_cursor_false_for_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = false;
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["plugin content"]);
    assert!(!app.has_text_cursor());
    fs::remove_dir_all(&root).ok();
}

// -- can_mouse_select tests --

#[test]
fn can_mouse_select_true_for_normal_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = false;
    assert!(app.can_mouse_select());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn can_mouse_select_false_for_diff() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert!(!app.can_mouse_select());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn can_mouse_select_true_for_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.is_diff = false;
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["plugin content"]);
    assert!(app.can_mouse_select());
    fs::remove_dir_all(&root).ok();
}

// -- set_active_line_from_physical tests --

#[test]
fn set_active_line_from_physical_is_identity_without_folds() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content = (0..20).map(|i| format!("line {i}")).collect();
    app.session_dirty = false;
    app.set_active_line_from_physical(7);
    assert_eq!(app.active_line, 7);
    assert!(app.session_dirty);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_active_line_from_physical_maps_through_fold_display_map() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content = (0..20).map(|i| format!("line {i}")).collect();
    // Display line 1 shows physical line 6 (physical lines 1..=5 are folded away).
    app.fold_display_map = vec![0, 6, 7, 8, 9, 10];
    app.set_active_line_from_physical(6);
    assert_eq!(
        app.active_line, 1,
        "active_line must be the display index, not the physical index"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_active_line_from_physical_clamps_to_display_line_count() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content = (0..20).map(|i| format!("line {i}")).collect();
    app.fold_display_map = vec![0, 1, 2];
    // Physical line 19 is past the end of the fold display map.
    app.set_active_line_from_physical(19);
    assert_eq!(app.active_line, app.display_line_count() - 1);
    fs::remove_dir_all(&root).ok();
}

// -- scroll_line_into_view tests --

#[test]
fn scroll_line_into_view_already_visible_noop() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.content_scroll = 10;
    app.scroll_line_into_view(12); // line 12 is within [10, 20)
    assert_eq!(app.content_scroll, 10);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_line_into_view_above_viewport() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.content_scroll = 20;
    app.scroll_line_into_view(5); // line 5 is above scroll
    assert_eq!(app.content_scroll, 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_line_into_view_below_viewport() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.content_scroll = 5;
    // viewport covers lines 5..15, line 30 is below
    app.scroll_line_into_view(30);
    // target = 30 - 10 + 1 = 21
    assert_eq!(app.content_scroll, 21);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_line_into_view_last_row() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.content_scroll = 0;
    // line 49 is the last, viewport is 10 tall → scroll to 40
    app.scroll_line_into_view(49);
    assert_eq!(app.content_scroll, 40);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_line_into_view_clamps_to_max() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    // scroll_line_into_view of line 49 with height 10 should not exceed max
    // max = 50 - 10 = 40
    app.scroll_line_into_view(49);
    assert_eq!(app.content_scroll, 40);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_blame_into_view_nudges_scroll_when_active_line_outside_viewport() {
    let root = temp_root();
    let mut app = app_for(&root);
    // Don't open_file — set content directly so line_count() reads app.content.
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 10,
    };
    app.content_scroll = 0;
    app.active_line = 15;
    // active_line (15) is past content_area height (10) from scroll (0).
    app.scroll_blame_into_view();
    assert!(
        app.content_scroll > 0,
        "content_scroll should advance so active_line is visible"
    );
    let vh = app.content_area.height as usize;
    assert!(
        app.active_line >= app.content_scroll && app.active_line < app.content_scroll + vh,
        "active_line={} should be within [content_scroll={}, {}]",
        app.active_line,
        app.content_scroll,
        app.content_scroll + vh
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_blame_into_view_does_not_nudge_when_already_in_view() {
    let root = temp_root();
    let mut app = app_for(&root);
    // Don't open_file — set content directly so line_count() reads app.content.
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.virtual_file = None;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 10,
    };
    app.content_scroll = 5;
    app.active_line = 7;
    app.scroll_blame_into_view();
    assert_eq!(app.content_scroll, 5, "content_scroll must not change");
    fs::remove_dir_all(&root).ok();
}
