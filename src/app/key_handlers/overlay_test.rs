use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::{GotoLineState, InFileSearch, ThemePicker, TreeFilter};
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
