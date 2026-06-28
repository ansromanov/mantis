use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::{GotoLineState, InFileSearch, TreeFilter};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_overlay_test_{}_{n}", std::process::id()));
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

#[test]
fn tree_filter_jump_scrolls_match_into_view() {
    let root = temp_tree();
    // Many files so the only match sits well below a short viewport.
    for i in 0..20 {
        fs::write(root.join(format!("f{i:02}.txt")), "").unwrap();
    }
    fs::write(root.join("zzz_target.txt"), "").unwrap();
    let mut app = app_for(&root);
    assert!(!app.tree_independent_scroll, "default mode under test");
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };
    app.tree_filter = Some(TreeFilter::new());

    // Type a query matching only the far-down file.
    for c in "zzz".chars() {
        app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }

    let sel = app.nodes[app.tree_selected].path.clone();
    assert!(
        sel.ends_with("zzz_target.txt"),
        "filter must select the matching node, got {sel:?}"
    );
    let h = app.tree_area.height as usize;
    assert!(
        app.tree_selected >= app.tree_scroll && app.tree_selected < app.tree_scroll + h,
        "filtered match {} must be within viewport [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + h
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_down_advances_current_match() {
    // Write multiple 'f'-containing lines so the search finds >=2 matches.
    let root = temp_tree();
    fs::write(root.join("a.txt"), "foo bar\nfoo baz\nfoo qux\n").unwrap();
    let mut app = app_for(&root);
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    let mut s = InFileSearch::new();
    s.push('f');
    app.in_file_search = Some(s);
    app.refresh_in_file_search();
    assert!(
        app.in_file_search.as_ref().unwrap().matches.len() >= 2,
        "need >=2 matches; got {}",
        app.in_file_search.as_ref().unwrap().matches.len()
    );
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    // Down should advance to next match without resetting to 0.
    app.handle_in_file_search_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 1);
    // Up should go back.
    app.handle_in_file_search_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_down_moves_to_next_match_and_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    let visible = vec![0usize, 2, 5];
    app.tree_visible_indices = Some(visible.clone());
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    // Start at first match.
    app.tree_selected = visible[0];

    // Down -> second match
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[1]);

    // Down -> third match
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[2]);

    // Down -> clamped at last
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[2]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_up_moves_to_prev_match_and_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    let visible = vec![0usize, 2, 5];
    app.tree_visible_indices = Some(visible.clone());
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    // Start at last match.
    app.tree_selected = visible[2];

    // Up -> second match
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[1]);

    // Up -> first match
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[0]);

    // Up -> clamped at first
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[0]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_pagedown_advances_by_page_and_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    // A large visible set so we can test page scrolling.
    let visible: Vec<usize> = (0..50).collect();
    app.tree_visible_indices = Some(visible.clone());
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.tree_selected = visible[0];

    // tree_area.height = 10, page = 10.max(1) = 10
    // PageDown from first -> index 10
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[10]);

    // PageDown again -> index 20
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[20]);

    // Jump near end then PageDown clamps
    app.tree_selected = visible[48];
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[49]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_pageup_goes_back_by_page_and_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    let visible: Vec<usize> = (0..50).collect();
    app.tree_visible_indices = Some(visible.clone());
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    // tree_area.height = 10, page = 10
    app.tree_selected = visible[30];

    // PageUp from 30 -> 20
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[20]);

    // PageUp again -> 10
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[10]);

    // PageUp from first clamps to first
    app.tree_selected = visible[0];
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, visible[0]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_typing_after_navigation_jumps_to_first_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    for c in "a".chars() {
        app.tree_filter.as_mut().unwrap().push(c);
    }
    let visible = vec![0usize, 1];
    app.tree_visible_indices = Some(visible.clone());
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };

    // Start at first match (index 0).
    assert_eq!(app.tree_selected, 0);

    // Navigate down to the second match (index 1).
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, 1);

    // Type another char — should re-jump to first match.
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_enter_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none());
    fs::remove_dir_all(&root).ok();
}
