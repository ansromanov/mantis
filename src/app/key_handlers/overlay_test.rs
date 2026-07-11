use crate::app::{App, Focus};
use crate::command_palette::{CommandPalette, COMMANDS};
use crate::config::Config;
use crate::search::{
    BugReportState, GotoLineState, InFileSearch, RevisionItem, RevisionPicker, SearchState,
    ThemePicker, TreeFilter,
};
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
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
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
    // pressing the open binding (ctrl+g) while the dialog is open should not
    // append to the query
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
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
fn handle_goto_line_key_activate_moves_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;
    let mut g = GotoLineState::new();
    g.query = "20".to_string();
    app.goto_line = Some(g);
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(
        app.active_line, 19,
        "1-indexed input lands on the 0-indexed line"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_activate_relative_plus_is_cursor_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 10;
    let mut g = GotoLineState::new();
    g.query = "+5".to_string();
    app.goto_line = Some(g);
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        app.active_line, 15,
        "+n offsets from active_line, not content_scroll"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_activate_relative_minus_is_cursor_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 10;
    let mut g = GotoLineState::new();
    g.query = "-3".to_string();
    app.goto_line = Some(g);
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        app.active_line, 7,
        "-n offsets from active_line, not content_scroll"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_activate_clamps_to_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;
    let mut g = GotoLineState::new();
    g.query = "9999".to_string();
    app.goto_line = Some(g);
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    let last = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_goto_line_key_activate_relative_uses_content_scroll_without_cursor() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.is_diff = true;
    app.active_line = 999; // stale cursor value from before the diff was shown
    app.content_scroll = 10;
    let mut g = GotoLineState::new();
    g.query = "+5".to_string();
    app.goto_line = Some(g);
    app.handle_goto_line_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        app.content_scroll, 15,
        "cursorless views offset from content_scroll, ignoring stale active_line"
    );
    assert_eq!(
        app.active_line, 999,
        "active_line is untouched in cursorless views"
    );
    fs::remove_dir_all(&root).ok();
}

// -- Revision picker ---------------------------------------------------------

#[test]
fn handle_revision_key_esc_closes_without_entering_compare_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.revision_picker = Some(RevisionPicker::new(&root));
    app.handle_revision_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.revision_picker.is_none());
    assert!(app.compare_base.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_char_appends_to_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.revision_picker = Some(RevisionPicker::new(&root));
    for c in "HEAD".chars() {
        app.handle_revision_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    assert_eq!(app.revision_picker.as_ref().unwrap().query, "HEAD");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_enter_with_empty_query_and_empty_items_closes_without_entering() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // A picker with no items and empty query: nothing to select and nothing typed.
    app.revision_picker = Some(RevisionPicker {
        items: vec![],
        query: String::new(),
        filtered: vec![],
        selected: 0,
        matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
    });
    app.handle_revision_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.revision_picker.is_none());
    assert!(
        app.compare_base.is_none(),
        "empty revision must not enter compare mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_enter_with_empty_query_selects_first_item() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Picker with shortcuts: pressing Enter selects HEAD.
    app.revision_picker = Some(RevisionPicker::new(&root));
    app.handle_revision_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.revision_picker.is_none());
    assert_eq!(app.compare_base.as_deref(), Some("HEAD"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_enter_with_selection_enters_compare_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Build a picker with a known item, bypassing git shell-out.
    app.revision_picker = Some(RevisionPicker {
        items: vec![RevisionItem {
            rev: "HEAD".into(),
            display: "HEAD (current)".into(),
        }],
        query: String::new(),
        filtered: vec![0],
        selected: 0,
        matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
    });
    app.handle_revision_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.revision_picker.is_none());
    assert_eq!(app.compare_base.as_deref(), Some("HEAD"));
    assert!(app.git_mode, "entering compare mode should enable git mode");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_enter_with_empty_filtered_list_uses_typed_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.revision_picker = Some(RevisionPicker {
        items: vec![RevisionItem {
            rev: "main".into(),
            display: "branch: main".into(),
        }],
        query: String::from("HEAD~3"),
        filtered: vec![],
        selected: 0,
        matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
    });
    app.handle_revision_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.revision_picker.is_none());
    assert_eq!(
        app.compare_base.as_deref(),
        Some("HEAD~3"),
        "when filtered list is empty, the typed query should be used as the revspec"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_revision_key_down_navigates_list() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.revision_picker = Some(RevisionPicker {
        items: vec![
            RevisionItem {
                rev: "HEAD".into(),
                display: "HEAD (current)".into(),
            },
            RevisionItem {
                rev: "main".into(),
                display: "branch: main".into(),
            },
        ],
        query: String::new(),
        filtered: vec![0, 1],
        selected: 0,
        matcher: fuzzy_matcher::skim::SkimMatcherV2::default(),
    });
    app.handle_revision_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.revision_picker.as_ref().unwrap().selected, 1);
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
fn tree_filter_j_types_when_query_non_empty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut f = TreeFilter::new();
    f.push('a'); // query non-empty
    app.tree_filter = Some(f);
    app.tree_selected = 0;
    // 'j' must append to query, not navigate
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(
        app.tree_filter.as_ref().map(|f| f.query.as_str()),
        Some("aj"),
        "'j' must append to query when query is non-empty"
    );
    assert_eq!(app.tree_selected, 0, "selection must not change");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_k_types_when_query_non_empty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut f = TreeFilter::new();
    f.push('a'); // query non-empty
    app.tree_filter = Some(f);
    app.tree_selected = 0;
    // 'k' must append to query, not navigate
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
    assert_eq!(
        app.tree_filter.as_ref().map(|f| f.query.as_str()),
        Some("ak"),
        "'k' must append to query when query is non-empty"
    );
    assert_eq!(app.tree_selected, 0, "selection must not change");
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
fn tree_filter_enter_on_file_opens_it() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // App::new opens the first file. Clear state so we can test Enter opens it.
    app.current_file = None;
    app.tree_filter = Some(TreeFilter::new());
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path.ends_with("a.txt"))
        .expect("a.txt must be in the tree");
    app.tree_selected = file_idx;
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    app.pump_loads();
    assert!(app.tree_filter.is_none(), "filter must close on Enter");
    assert_eq!(
        app.current_file.as_deref(),
        Some(root.join("a.txt").as_path())
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_enter_on_dir_toggles_expansion() {
    let root = temp_tree();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub").join("c.txt"), "nested\n").unwrap();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path.ends_with("sub"))
        .expect("sub dir must be in the tree");
    app.tree_selected = dir_idx;
    assert!(
        !app.expanded.contains(&root.join("sub")),
        "sub must not be expanded initially"
    );
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none(), "filter must close on Enter");
    assert!(
        app.expanded.contains(&root.join("sub")),
        "sub must be expanded after Enter"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_typing_finds_match_in_collapsed_subdirectory() {
    let root = temp_tree();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub").join("needle.txt"), "hidden\n").unwrap();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    assert!(
        !app.expanded.contains(&root.join("sub")),
        "sub must not be expanded initially"
    );
    for c in "needle".chars() {
        app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    assert!(
        app.expanded.contains(&root.join("sub")),
        "typing a query matching a file in a collapsed dir must auto-expand it"
    );
    assert!(
        app.nodes.iter().any(|n| n.path.ends_with("needle.txt")),
        "the matching file must now be present in the rebuilt node list"
    );
    let sel = &app.nodes[app.tree_selected];
    assert!(
        sel.path.ends_with("needle.txt"),
        "selection must jump to the match"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_esc_restores_previous_expansion_state() {
    let root = temp_tree();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub").join("needle.txt"), "hidden\n").unwrap();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    for c in "needle".chars() {
        app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    assert!(app.expanded.contains(&root.join("sub")));
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none());
    assert!(
        !app.expanded.contains(&root.join("sub")),
        "dismissing the filter must restore the pre-filter expansion state"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_backspace_to_empty_restores_previous_expansion_state() {
    let root = temp_tree();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("sub").join("needle.txt"), "hidden\n").unwrap();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert!(app.expanded.contains(&root.join("sub")));
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert!(
        !app.expanded.contains(&root.join("sub")),
        "backspacing the query back to empty must restore prior expansion"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_esc_does_not_activate() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // App::new opens the first file; clear so we can test Esc doesn't open one.
    app.current_file = None;
    app.tree_filter = Some(TreeFilter::new());
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path.ends_with("a.txt"))
        .expect("a.txt must be in the tree");
    app.tree_selected = file_idx;
    app.handle_tree_filter_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none(), "filter must close on Esc");
    assert!(app.current_file.is_none(), "Esc must not open a file");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_regex_matches_files() {
    let root = temp_tree();
    // Create files with distinct patterns
    fs::write(root.join("main.rs"), "").unwrap();
    fs::write(root.join("main_test.rs"), "").unwrap();
    fs::write(root.join("readme.md"), "").unwrap();
    // Rebuild the app so it sees the new files
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());

    // Type a regex that matches .rs files only, not .md
    for c in r"\.rs$".chars() {
        app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }

    // Selection should land on the first .rs file (main.rs sorted before main_test.rs).
    // `\.rs$` should match .rs files but not .md.
    let sel = &app.nodes[app.tree_selected];
    assert!(
        sel.path.ends_with("main.rs"),
        "regex '\\.rs$' should select main.rs, got {:?}",
        sel.path
    );

    // Verify the filter's matches_name works correctly for each node.
    let filter = app.tree_filter.as_ref().unwrap();
    assert!(filter.matches_name("main.rs"), "regex should match main.rs");
    assert!(
        filter.matches_name("main_test.rs"),
        "regex should match main_test.rs"
    );
    assert!(
        !filter.matches_name("readme.md"),
        "regex should NOT match readme.md"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_invalid_regex_falls_back_to_substring() {
    let root = temp_tree();
    fs::write(root.join("file[1].txt"), "").unwrap();
    fs::write(root.join("file1.txt"), "").unwrap();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());

    // Unclosed bracket is not a valid regex, falls back to substring match
    for c in "[1".chars() {
        app.handle_tree_filter_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }

    // Substring "[1" matches "file[1].txt" but not "file1.txt"
    let sel = &app.nodes[app.tree_selected];
    assert!(
        sel.path.ends_with("file[1].txt"),
        "fallback substring should match file[1].txt, got {:?}",
        sel.path
    );
    fs::remove_dir_all(&root).ok();
}

// -- Theme picker live preview -----------------------------------------------

#[test]
fn handle_theme_key_down_keeps_picker_open_for_preview() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(ThemePicker::default());
    let before = app.theme_picker.as_ref().unwrap().selected;
    app.handle_theme_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert!(
        app.theme_picker.is_some(),
        "picker must stay open after navigation preview"
    );
    let after = app.theme_picker.as_ref().unwrap().selected;
    assert_ne!(after, before, "selection must advance on Down");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_up_clamps_and_does_not_close() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(ThemePicker::default());
    // Up at index 0 should clamp, not close
    app.handle_theme_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert!(
        app.theme_picker.is_some(),
        "picker must stay open after Up at top"
    );
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_down_previews_selected_theme_colors() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(ThemePicker::default());
    app.handle_theme_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    let picker = app.theme_picker.as_ref().unwrap();
    let previewed_name = picker.selected_name().unwrap().to_string();
    let previewed = picker.selected_theme().unwrap().clone();
    assert_eq!(
        app.theme.accent, previewed.accent,
        "navigating the picker must apply the previewed theme's colors live"
    );
    assert_ne!(
        app.config.theme.name.as_deref(),
        Some(previewed_name.as_str()),
        "preview must not write the config theme until Enter commits it"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_esc_closes_and_reverts() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let original_accent = app.theme.accent;
    let original_config_name = app.config.theme.name.clone();
    app.theme_picker = Some(ThemePicker::default());
    // Navigate to create a preview state first
    app.handle_theme_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert!(
        app.theme_picker.is_some(),
        "picker must stay open after preview"
    );
    // Esc should close and revert
    app.handle_theme_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.theme_picker.is_none(), "picker must close on Esc");
    assert_eq!(
        app.theme.accent, original_accent,
        "Esc must restore the theme that was active before the picker opened"
    );
    assert_eq!(
        app.config.theme.name, original_config_name,
        "Esc must not have written the previewed theme to config"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_esc_falls_back_to_default_for_unloadable_config_theme() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // A config theme name that doesn't resolve to any bundled/user theme.
    app.config.theme.name = Some("does-not-exist".to_string());
    let default_accent = crate::theme::Theme::load("default").unwrap().accent;
    app.theme_picker = Some(ThemePicker::default());
    app.handle_theme_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    app.handle_theme_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.theme_picker.is_none(), "picker must close on Esc");
    assert_eq!(
        app.theme.accent, default_accent,
        "reverting to an unloadable config theme must fall back to the default theme"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_enter_commits_and_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(ThemePicker::default());
    // Filter to "default" for a predictable selection
    for c in "default".chars() {
        app.handle_theme_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    let picker = app.theme_picker.as_ref().unwrap();
    assert_eq!(
        picker.selected_name(),
        Some("default"),
        "typing 'default' should select the default theme"
    );
    app.handle_theme_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.theme_picker.is_none(), "picker must close on Enter");
    assert_eq!(
        app.config.theme.name.as_deref(),
        Some("default"),
        "Enter must commit theme to config"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_theme_key_query_typing_previews_first_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.theme_picker = Some(ThemePicker::default());
    // Typing narrows the list and resets selection — the first match is previewed.
    // "monokai" is a bundled theme; use a query short enough to match something.
    for c in "monokai".chars() {
        app.handle_theme_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    assert!(
        app.theme_picker.is_some(),
        "picker must stay open during query typing"
    );
    // After typing, the first (selected) item should start with "monokai".
    let picker = app.theme_picker.as_ref().unwrap();
    assert_eq!(
        picker.selected_name(),
        Some("monokai"),
        "typing 'monokai' should match monokai"
    );
    let previewed = picker.selected_theme().unwrap().clone();
    assert_eq!(
        app.theme.accent, previewed.accent,
        "query typing must preview the newly matched theme's colors live"
    );
    fs::remove_dir_all(&root).ok();
}

/// Pressing Enter on a command-palette selection dispatches it through
/// `dispatch_command`. Uses "quit" as the probed action since it flips a
/// simple, unambiguous flag (`should_quit`), which stayed keymap-only until
/// this PR added its palette entry (issue #495).
#[test]
fn handle_command_key_enter_dispatches_selected_command() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "quit")
        .expect("quit must be in COMMANDS");
    app.command_palette = Some(CommandPalette::default());
    if let Some(p) = &mut app.command_palette {
        p.filtered = vec![idx];
        p.selected = 0;
    }
    assert!(!app.should_quit);
    app.handle_command_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(
        app.should_quit,
        "Enter on the 'quit' palette entry must dispatch it"
    );
    assert!(
        app.command_palette.is_none(),
        "dispatching a command must close the palette"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_search_key_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut s = SearchState::new(&root, false, true, 0, None);
    // The toggles only affect Content-mode search (see refresh_content), so
    // switch out of the default Files mode before exercising them.
    s.toggle_mode();
    app.search = Some(s);

    let regex_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(!app.search.as_ref().unwrap().regex);
    app.handle_search_key(regex_key);
    assert!(app.search.as_ref().unwrap().regex);

    let case_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert!(!app.search.as_ref().unwrap().case_sensitive);
    app.handle_search_key(case_key);
    assert!(app.search.as_ref().unwrap().case_sensitive);

    let word_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert!(!app.search.as_ref().unwrap().whole_word);
    app.handle_search_key(word_key);
    assert!(app.search.as_ref().unwrap().whole_word);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_search_key_unmodified_letters_type_into_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.search = Some(SearchState::new(&root, false, true, 0, None));

    app.handle_search_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()));
    let s = app.search.as_ref().unwrap();
    assert_eq!(s.query, "r");
    assert!(!s.regex);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_search_key_toggles_are_ignored_in_files_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Default mode is Files; the toggles only affect Content search, so
    // pressing them here must be a no-op (state unchanged, selection intact).
    let mut s = SearchState::new(&root, false, true, 0, None);
    s.selected = 3;
    app.search = Some(s);

    app.handle_search_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL));
    let s = app.search.as_ref().unwrap();
    assert!(!s.regex);
    assert_eq!(
        s.selected, 3,
        "toggle must not reset selection in Files mode"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_in_file_search_key_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());

    let regex_key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
    assert!(!app.in_file_search.as_ref().unwrap().regex);
    app.handle_in_file_search_key(regex_key);
    assert!(app.in_file_search.as_ref().unwrap().regex);

    let case_key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);
    assert!(!app.in_file_search.as_ref().unwrap().case_sensitive);
    app.handle_in_file_search_key(case_key);
    assert!(app.in_file_search.as_ref().unwrap().case_sensitive);

    let word_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert!(!app.in_file_search.as_ref().unwrap().whole_word);
    app.handle_in_file_search_key(word_key);
    assert!(app.in_file_search.as_ref().unwrap().whole_word);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_bug_report_key_handling() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK.lock().unwrap();
    let state_dir = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state_dir.path());

    let root = temp_tree();
    let mut app = app_for(&root);
    app.bug_report = Some(BugReportState::default());

    // Type "Hi"
    app.handle_bug_report_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.handle_bug_report_key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::empty()));
    assert_eq!(
        app.bug_report.as_ref().unwrap().text,
        vec!["Hi".to_string()]
    );

    // Press Enter (newline)
    app.handle_bug_report_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert_eq!(
        app.bug_report.as_ref().unwrap().text,
        vec!["Hi".to_string(), "".to_string()]
    );

    // Type "there"
    for c in "there".chars() {
        app.handle_bug_report_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    assert_eq!(
        app.bug_report.as_ref().unwrap().text,
        vec!["Hi".to_string(), "there".to_string()]
    );

    // Test Backspace
    app.handle_bug_report_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(
        app.bug_report.as_ref().unwrap().text,
        vec!["Hi".to_string(), "ther".to_string()]
    );

    // Test Ctrl+S to submit & save
    app.handle_bug_report_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    assert!(app.bug_report.is_none());

    let saved: Vec<_> = fs::read_dir(state_dir.path().join("bug-reports"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(saved.len(), 1);

    std::env::remove_var("MANTIS_STATE_DIR");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_bug_report_key_esc_closes_modal() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.bug_report = Some(BugReportState::default());

    app.handle_bug_report_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.bug_report.is_none());

    fs::remove_dir_all(&root).ok();
}

// -- command palette prefix routing --------------------------------------------

#[test]
fn palette_slash_prefix_creates_file_sub_picker() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    app.handle_command_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    let p = app.command_palette.as_ref().unwrap();
    assert_eq!(p.route, crate::command_palette::PaletteRoute::Files);
    assert!(
        p.route_search.is_some(),
        "typing the prefix must create the file sub-picker"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn palette_tab_toggles_files_and_content_both_ways() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    app.handle_command_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_command_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(
        app.command_palette.as_ref().unwrap().route,
        crate::command_palette::PaletteRoute::Content,
        "Tab in the files route switches to content"
    );
    app.handle_command_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(
        app.command_palette.as_ref().unwrap().route,
        crate::command_palette::PaletteRoute::Files,
        "Tab in the content route switches back to files"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn palette_esc_in_routed_mode_returns_to_commands_then_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    app.handle_command_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_command_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    let p = app.command_palette.as_ref().expect("palette stays open");
    assert_eq!(p.route, crate::command_palette::PaletteRoute::Commands);
    assert!(p.route_search.is_none());
    app.handle_command_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(
        app.command_palette.is_none(),
        "second Esc closes the palette"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn palette_goto_line_route_enter_jumps_and_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;
    app.command_palette = Some(CommandPalette::default());
    app.handle_command_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app
        .command_palette
        .as_ref()
        .unwrap()
        .route_goto_line
        .is_some());
    app.handle_command_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));
    app.handle_command_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::empty()));
    app.handle_command_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.command_palette.is_none(), "Enter closes the palette");
    assert_eq!(
        app.active_line, 19,
        "palette goto-line matches the standalone dialog"
    );
    fs::remove_dir_all(&root).ok();
}

// -- repo log overlay ---------------------------------------------------------

fn repo_log_with_commits(root: PathBuf, n: usize) -> crate::search::RepoLogState {
    let mut s = crate::search::RepoLogState::new(root);
    s.commits = (0..n)
        .map(|i| crate::git::Commit {
            hash: format!("{i:040}"),
            short: format!("{i:07}"),
            date: "2024-01-01".to_string(),
            author: "Test".to_string(),
            subject: format!("commit {i}"),
        })
        .collect();
    s.filtered = (0..n).collect();
    s.selected = 0;
    s
}

#[test]
fn handle_repo_log_key_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.repo_log = Some(repo_log_with_commits(root.clone(), 2));
    app.handle_repo_log_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.repo_log.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_repo_log_key_enter_enters_compare_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut s = repo_log_with_commits(root.clone(), 2);
    s.selected = 1;
    app.repo_log = Some(s);
    app.handle_repo_log_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.repo_log.is_none());
    assert_eq!(app.compare_base, Some(format!("{:040}", 1)));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_repo_log_key_char_appends_to_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.repo_log = Some(repo_log_with_commits(root.clone(), 2));
    app.handle_repo_log_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::empty()));
    assert_eq!(app.repo_log.as_ref().unwrap().query, "c");
    fs::remove_dir_all(&root).ok();
}

/// With a non-empty query 'j' is a query character: it must append, not
/// navigate, and must not trigger paging.
#[test]
fn handle_repo_log_key_j_with_query_is_text_not_paging() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut s = repo_log_with_commits(root.clone(), 2);
    s.push('c'); // non-empty query; both fabricated subjects match "c"
    app.repo_log = Some(s);
    app.handle_repo_log_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(app.repo_log.as_ref().unwrap().query, "cj");
    fs::remove_dir_all(&root).ok();
}
