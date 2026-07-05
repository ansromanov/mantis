use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::App;
use crate::config::Config;
use crate::list_picker::ListPicker;

use super::{handle_picker_mouse, PickerMouseAction};

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

fn scroll_up_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
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
fn scroll_up_at_content_top_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 40,
        y: 0,
        width: 40,
        height: 10,
    };
    assert_eq!(app.content_scroll, 0, "precondition: starts at top");
    app.session_dirty = false;
    app.handle_mouse(scroll_up_at(50, 5));
    assert_eq!(
        app.content_scroll, 0,
        "wheel-up at the first line must stay at the top"
    );
    assert!(
        !app.session_dirty,
        "wheel-up at a bound must not mark the session dirty since scroll state did not change"
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
fn help_overlay_intercepts_wheel_before_content_pane() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.show_help = true;
    app.handle_mouse(scroll_down_at(40, 5));
    assert_eq!(
        app.help_scroll, 3,
        "wheel scroll while the help overlay is open must move help_scroll, not content_scroll"
    );
    assert_eq!(
        app.content_scroll, 0,
        "help overlay must intercept the wheel event before it reaches the content pane"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn help_overlay_wheel_up_does_not_underflow() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_help = true;
    app.handle_mouse(scroll_up_at(40, 5));
    assert_eq!(
        app.help_scroll, 0,
        "wheel-up at the top of the help overlay must saturate at zero"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn help_overlay_mouse_click_actions() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_help = true;
    app.help_area = Rect {
        x: 10,
        y: 10,
        width: 80,
        height: 20,
    };
    app.help_tab = 0;

    // Left click outside: should close help overlay and reset tab
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 5,
        row: 5,
        modifiers: crossterm::event::KeyModifiers::empty(),
    });
    assert!(!app.show_help);
    assert_eq!(app.help_tab, 0);

    // Reopen help
    app.show_help = true;
    // Click on Tab 1 "Navigation" (ranges are computed from help_area.x + 1 which is 11)
    let ranges = crate::ui::popups::help_tab_ranges(11);
    let nav_tab_x = ranges[1].0;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: nav_tab_x,
        row: 11, // help_area.y + 1
        modifiers: crossterm::event::KeyModifiers::empty(),
    });
    assert_eq!(app.help_tab, 1);

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
fn breadcrumb_double_click_on_compact_dotdot_changes_root() {
    let root = tree_with_dir();
    let mut app = app_for(&root);
    let parent = root.parent().expect("temp dir has a parent").to_path_buf();

    // Simulate a compact breadcrumb: a ".." segment pointing to the parent,
    // followed by the root segment.
    app.breadcrumb_areas.push((
        parent.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 2,
            height: 1,
        },
    ));
    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 6,
            y: 1,
            width: 4,
            height: 1,
        },
    ));

    // Double-click on the ".." rect.
    app.last_breadcrumb_click = Some((Instant::now(), parent.clone()));
    app.handle_mouse(left_down_at(2, 1));

    assert_eq!(
        app.root, parent,
        "double-click on compact .. must change root to parent"
    );
    assert!(
        app.last_breadcrumb_click.is_none(),
        "last_breadcrumb_click must be cleared after navigation"
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
fn mouse_content_click_moves_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 10,
    };
    app.content_scroll = 3;
    app.active_line = 0;
    // Click at row 5 + 4 -> content row 4 -> physical line = scroll(3) + 4 = 7
    app.handle_mouse(left_down_at(6, 5 + 4));
    assert_eq!(app.active_line, 7);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_content_click_moves_active_line_in_display_space_when_folded() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 10,
    };
    // Simulate a fold hiding physical lines 1..=5: display line 1 -> physical line 6.
    app.fold_display_map = vec![0, 6, 7, 8, 9, 10];
    app.content_scroll = 0;
    app.active_line = 0;
    app.session_dirty = false;
    // Click at content row 1 -> display line 1 -> physical line 6 -> display line 1.
    app.handle_mouse(left_down_at(6, 5 + 1));
    assert_eq!(
        app.active_line, 1,
        "active_line must stay in display space, not the physical line index"
    );
    assert!(
        app.session_dirty,
        "moving the cursor via click must mark the session dirty, like keyboard navigation does"
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

#[test]
fn splitter_drag_release_saves_tree_width_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Simulate a splitter drag: set splitter_drag and adjust tree_width, then release.
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 28,
        height: 40,
    };
    app.content_area = Rect {
        x: 32,
        y: 0,
        width: 48,
        height: 40,
    };
    app.splitter_drag = true;
    app.tree_width = 35;
    // Mouse up ends the drag and persists to config.
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 35,
        row: 5,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse(up);
    assert!(!app.splitter_drag, "drag flag should clear on release");
    assert_eq!(
        app.config.tree.width, 35,
        "config.tree.width should reflect the new tree_width"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_click_out_of_range_is_noop() {
    let root = tree_with_dir();
    let mut app = app_for(&root);
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 10,
    };
    app.tree_offset = 0;
    let prev = app.tree_selected;
    // Click on row 100, well past the end of the node list.
    app.handle_mouse(left_down_at(1, 100));
    assert_eq!(
        app.tree_selected, prev,
        "click on out-of-range row must not change selection"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_picker_click_outside_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.plugin_picker = Some(crate::search::PluginPicker::new(vec![]));
    app.plugin_picker_area = Rect {
        x: 10,
        y: 10,
        width: 40,
        height: 20,
    };
    app.handle_mouse(left_down_at(1, 1)); // outside popup
    assert!(app.plugin_picker.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- handle_picker_mouse unit tests --

struct FakePicker {
    len: usize,
    selected: usize,
}

impl ListPicker for FakePicker {
    fn query_push(&mut self, _c: char) {}
    fn query_pop(&mut self) {}
    fn query_is_empty(&self) -> bool {
        true
    }
    fn results_len(&self) -> usize {
        self.len
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
    }
}

fn picker_area() -> Rect {
    Rect {
        x: 10,
        y: 10,
        width: 40,
        height: 20,
    }
}

fn ev_scroll_down_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn ev_scroll_up_at(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

#[test]
fn picker_click_outside_closes_picker() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 0,
    });
    let mut last_click = None;
    let action = handle_picker_mouse(
        left_down_at(0, 0), // outside picker_area
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(action, PickerMouseAction::None);
    assert!(picker.is_none(), "click outside area must close the picker");
}

#[test]
fn picker_single_click_inside_selects_row() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 0,
    });
    let mut last_click = None;
    // area.y = 10; row 12 → index = offset(0) + (12 - 10) = 2
    let action = handle_picker_mouse(
        left_down_at(15, 12),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(action, PickerMouseAction::None);
    assert_eq!(picker.as_ref().unwrap().selected, 2);
    assert!(
        last_click.is_some(),
        "single click must arm double-click detection"
    );
}

#[test]
fn picker_double_click_returns_activate() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 0,
    });
    // Pre-arm: first click on index 2 just happened.
    let mut last_click: Option<(Instant, usize)> = Some((Instant::now(), 2));
    let action = handle_picker_mouse(
        left_down_at(15, 12), // same row → index 2 again
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(action, PickerMouseAction::Activate);
    assert!(
        last_click.is_none(),
        "last_click must clear after double-click"
    );
}

#[test]
fn picker_double_click_expired_is_single_click() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 0,
    });
    // Stale first click (600 ms ago — past the 400 ms window).
    let mut last_click: Option<(Instant, usize)> =
        Some((Instant::now() - Duration::from_millis(600), 2));
    let action = handle_picker_mouse(
        left_down_at(15, 12),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(
        action,
        PickerMouseAction::None,
        "expired double-click must not activate"
    );
    assert!(
        last_click.is_some(),
        "new single click must re-arm the timer"
    );
}

#[test]
fn picker_scroll_down_increments_selected() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 2,
    });
    let mut last_click = None;
    handle_picker_mouse(
        ev_scroll_down_at(0, 0),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(picker.unwrap().selected, 3);
}

#[test]
fn picker_scroll_down_clamps_at_last_item() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 4,
    });
    let mut last_click = None;
    handle_picker_mouse(
        ev_scroll_down_at(0, 0),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(
        picker.unwrap().selected,
        4,
        "scroll down at end must not overflow"
    );
}

#[test]
fn picker_scroll_up_decrements_selected() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 3,
    });
    let mut last_click = None;
    handle_picker_mouse(
        ev_scroll_up_at(0, 0),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(picker.unwrap().selected, 2);
}

#[test]
fn picker_scroll_up_clamps_at_zero() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 5,
        selected: 0,
    });
    let mut last_click = None;
    handle_picker_mouse(
        ev_scroll_up_at(0, 0),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(
        picker.unwrap().selected,
        0,
        "scroll up at first item must not underflow"
    );
}

#[test]
fn picker_click_in_range_with_offset_selects_correct_index() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 20,
        selected: 0,
    });
    let mut last_click = None;
    // offset = 5, area.y = 10, click row = 13 → index = 5 + (13 - 10) = 8
    handle_picker_mouse(
        left_down_at(15, 13),
        picker_area(),
        5,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(
        picker.unwrap().selected,
        8,
        "offset must be added to clicked row index"
    );
}

#[test]
fn picker_click_out_of_range_row_is_noop() {
    let mut picker: Option<FakePicker> = Some(FakePicker {
        len: 3,
        selected: 0,
    });
    let mut last_click = None;
    // area.y = 10, click row = 29 → index = 0 + 19 = 19, but len = 3 → out of range
    let action = handle_picker_mouse(
        left_down_at(15, 29),
        picker_area(),
        0,
        &mut picker,
        &mut last_click,
    );
    assert_eq!(action, PickerMouseAction::None);
    assert_eq!(
        picker.unwrap().selected,
        0,
        "out-of-range click must not change selection"
    );
}

#[test]
fn mouse_click_in_plugin_content_starts_drag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    app.open_file(&path);
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 20,
    };

    let text = vec![
        "plugin content line 1".to_string(),
        "plugin content line 2".to_string(),
    ];
    let style = ratatui::style::Style::default();
    let rendered = vec![
        vec![(style, "plugin content line 1".to_string())],
        vec![(style, "plugin content line 2".to_string())],
    ];
    app.plugin_content_text.insert(path.clone(), text);
    app.plugin_content.insert(path.clone(), rendered);

    let down_ev = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 6,
        row: 6,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse(down_ev);
    assert_eq!(app.focus, crate::app::Focus::Content);
    assert!(app.drag_start.is_some());

    let drag_ev = MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: 10,
        row: 6,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse(drag_ev);
    assert!(app.selection.is_some());

    let up_ev = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 10,
        row: 6,
        modifiers: crossterm::event::KeyModifiers::empty(),
    };
    app.handle_mouse(up_ev);
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

