use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::SearchMode;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_normal_key_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

// -- active-line navigation --------------------------------------------------

#[test]
fn content_gg_resets_active_line_to_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 10;
    app.handle_key(key(KeyCode::Char('g')));
    app.handle_key(key(KeyCode::Char('g')));
    assert_eq!(app.active_line, 0);
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
#[allow(non_snake_case)]
fn content_G_marks_session_dirty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.session_dirty = false;
    app.handle_key(key(KeyCode::Char('G')));
    assert!(
        app.active_line > 0,
        "precondition: G must move the active line"
    );
    assert!(
        app.session_dirty,
        "moving the active line in content must mark the session dirty so scroll position persists"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
#[allow(non_snake_case)]
fn content_G_moves_active_line_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('G')));
    let last = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_gg_in_diff_mode_scrolls_to_top() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.content_scroll = 5;
    app.handle_key(key(KeyCode::Char('g')));
    app.handle_key(key(KeyCode::Char('g')));
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
#[allow(non_snake_case)]
fn content_G_in_diff_mode_scrolls_to_bottom() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.content_scroll = 0;
    app.handle_key(key(KeyCode::Char('G')));
    assert_eq!(app.content_scroll, app.content_scroll_max());
    fs::remove_dir_all(&root).ok();
}

// -- blame_line toggle -------------------------------------------------------

#[test]
fn blame_line_key_toggles_show_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    assert!(!app.show_line_blame);
    app.handle_key(key(KeyCode::Char('B')));
    assert!(app.show_line_blame);
    app.handle_key(key(KeyCode::Char('B')));
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn blame_line_key_noop_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.handle_key(key(KeyCode::Char('B')));
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn blame_line_key_does_not_change_hscroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_hscroll = 8;
    app.handle_key(key(KeyCode::Char('B')));
    assert_eq!(app.content_hscroll, 8);
    fs::remove_dir_all(&root).ok();
}

// -- Esc dismisses blame before selection ------------------------------------

#[test]
fn esc_closes_line_blame_first() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn non_esc_does_not_close_line_blame() {
    // is_close maps only Esc; other keys must not dismiss blame.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_line_blame = true;
    // 'q' navigates in normal mode (not an Esc alias) — blame must stay open.
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
    assert!(app.show_line_blame, "non-Esc key must not close line blame");
    fs::remove_dir_all(&root).ok();
}

// -- navigation preserves blame popup ---------------------------------------

#[test]
fn nav_up_keeps_line_blame_and_updates_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 5;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('k')));
    assert!(app.show_line_blame, "blame popup stays open on nav up");
    assert_eq!(app.active_line, 4, "active line moves up");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn nav_down_keeps_line_blame_and_updates_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 5;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.show_line_blame, "blame popup stays open on nav down");
    assert_eq!(app.active_line, 6, "active line moves down");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_gg_keeps_line_blame_and_resets_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 10;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('g')));
    app.handle_key(key(KeyCode::Char('g')));
    assert!(app.show_line_blame, "blame popup stays open on gg");
    assert_eq!(app.active_line, 0, "active line resets to top");
    fs::remove_dir_all(&root).ok();
}

#[test]
#[allow(non_snake_case)]
fn content_G_keeps_line_blame_and_moves_to_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('G')));
    assert!(app.show_line_blame, "blame popup stays open on G");
    let last = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, last, "active line moves to last");
    fs::remove_dir_all(&root).ok();
}

// -- goto_line keybinding ----------------------------------------------------

#[test]
fn goto_line_keybinding_opens_dialog_with_content_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char(':')));
    assert!(app.goto_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_keybinding_is_noop_with_tree_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(key(KeyCode::Char(':')));
    assert!(app.goto_line.is_none());
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("go to line: switch to the content pane (Tab)")
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_status_clears_on_valid_keypress() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(key(KeyCode::Char(':')));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("go to line: switch to the content pane (Tab)")
    );
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.status_message.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- tree Home/End scroll selection into view (default, non-independent) ------

#[test]
fn tree_end_then_home_keeps_selection_in_view() {
    let root = temp_tree();
    // Enough files to overflow a short viewport.
    for i in 0..20 {
        fs::write(root.join(format!("f{i:02}.txt")), "").unwrap();
    }
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    assert!(!app.tree_independent_scroll, "default mode under test");
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };
    assert!(app.nodes.len() > 3, "tree must overflow the viewport");

    // End jumps to the last node; the viewport must follow so it stays visible.
    app.handle_key(key(KeyCode::End));
    let last = app.nodes.len() - 1;
    assert_eq!(app.tree_selected, last);
    let h = app.tree_area.height as usize;
    assert!(
        app.tree_selected >= app.tree_scroll && app.tree_selected < app.tree_scroll + h,
        "End selection {} must be within viewport [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + h
    );

    // Home returns to the top and resets the viewport.
    app.handle_key(key(KeyCode::Home));
    assert_eq!(app.tree_selected, 0);
    assert_eq!(app.tree_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn backspace_in_tree_calls_tree_up_dir() {
    let root = temp_tree();
    let orig_root = root.clone();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("long.txt"))
        .expect("long.txt");
    app.tree_selected = file_idx;
    let parent = root.parent().expect("root has parent").to_path_buf();
    app.handle_key(key(KeyCode::Backspace));
    assert_eq!(
        app.root, parent,
        "Backspace must call tree_up_dir and change root"
    );
    fs::remove_dir_all(&orig_root).ok();
}

// -- context-locked key feedback ---------------------------------------------

#[test]
fn git_mode_flat_toggle_noop_outside_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = false;
    app.handle_key(key(KeyCode::Char('F')));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("flat view: only in git mode (Ctrl+G)")
    );
    assert!(!app.git_mode_flat);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_pretty_json_noop_on_non_json() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    assert!(!app.is_json);
    app.handle_key(key(KeyCode::Char('J')));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("pretty JSON: not a JSON file")
    );
    assert!(!app.show_pretty_json);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_blame_noop_in_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.handle_key(key(KeyCode::Char('b')));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("blame: not available in a diff")
    );
    assert!(!app.show_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_flat_toggle_status_clears_on_valid_keypress() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = false;
    app.handle_key(key(KeyCode::Char('F')));
    assert!(app.status_message.is_some());
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.status_message.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_pretty_json_status_clears_on_valid_keypress() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('J')));
    assert!(app.status_message.is_some());
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.status_message.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_blame_status_clears_on_valid_keypress() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.handle_key(key(KeyCode::Char('b')));
    assert!(app.status_message.is_some());
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.status_message.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- diff toggle persistence -------------------------------------------------

#[test]
fn toggle_diff_side_by_side_persists_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.diff_side_by_side = false;
    app.handle_key(key(KeyCode::Char('D')));
    assert!(app.diff_side_by_side, "app field should toggle");
    assert!(
        app.config.git.diff.side_by_side,
        "config should persist the toggle"
    );
    // Toggle back.
    app.handle_key(key(KeyCode::Char('D')));
    assert!(!app.diff_side_by_side);
    assert!(!app.config.git.diff.side_by_side);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_diff_staged_persists_to_config() {
    use crate::app::DiffMode;
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.diff_mode = DiffMode::All;
    app.handle_key(key(KeyCode::Char('S')));
    assert_eq!(app.diff_mode, DiffMode::Staged, "should cycle to Staged");
    assert_eq!(
        app.config.git.diff.mode,
        DiffMode::Staged,
        "config should persist the new mode"
    );
    // Cycle again.
    app.handle_key(key(KeyCode::Char('S')));
    assert_eq!(app.diff_mode, DiffMode::Unstaged);
    assert_eq!(app.config.git.diff.mode, DiffMode::Unstaged);
    fs::remove_dir_all(&root).ok();
}

// -- content config persist --------------------------------------------------

#[test]
fn toggle_wrap_persists_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.word_wrap = false;
    app.handle_key(key(KeyCode::Char('z')));
    assert!(app.word_wrap, "app field should toggle");
    assert!(
        app.config.content.word_wrap,
        "config.content.word_wrap should persist the toggle"
    );
    app.handle_key(key(KeyCode::Char('z')));
    assert!(!app.word_wrap);
    assert!(!app.config.content.word_wrap);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_line_numbers_persists_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    let initial = app.show_line_numbers;
    app.handle_key(key(KeyCode::Char('L')));
    assert_eq!(app.show_line_numbers, !initial, "app field should toggle");
    assert_eq!(
        app.config.content.line_numbers, !initial,
        "config.content.line_numbers should persist the toggle"
    );
    fs::remove_dir_all(&root).ok();
}

// -- copy path ---------------------------------------------------------------

#[test]
fn copy_path_nothing_selected() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.focus = Focus::Content;
    app.copy_path_to_clipboard(false);
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("nothing selected")
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_nothing_selected_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.focus = Focus::Content;
    app.copy_path_to_clipboard(true);
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("nothing selected")
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_directory_from_tree_absolute() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let dir_path = app.nodes[0].path.clone();
    app.copy_path_to_clipboard(false);
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = cb.get_text() else { return };
    assert_eq!(text, dir_path.display().to_string());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_directory_from_tree_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let rel = app.nodes[0]
        .path
        .strip_prefix(&app.root)
        .unwrap()
        .display()
        .to_string();
    app.copy_path_to_clipboard(true);
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = cb.get_text() else { return };
    assert_eq!(text, rel);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_file_from_content_still_works() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.copy_path_to_clipboard(false);
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = cb.get_text() else { return };
    assert_eq!(text, root.join("long.txt").display().to_string());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_file_from_content_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.copy_path_to_clipboard(true);
    let Ok(mut cb) = arboard::Clipboard::new() else {
        return;
    };
    let Ok(text) = cb.get_text() else { return };
    assert_eq!(text, "long.txt");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_flat_toggle_flips_flag_when_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = true;
    assert!(!app.git_mode_flat);
    app.handle_key(key(KeyCode::Char('F')));
    assert!(app.git_mode_flat, "F key must enable flat mode in git mode");
    fs::remove_dir_all(&root).ok();
}

// -- viewing_revision key handlers -------------------------------------------

#[test]
fn esc_clears_viewing_revision_in_normal_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.viewing_revision = Some("abc1234".to_string());
    app.handle_key(key(KeyCode::Esc));
    assert!(
        app.viewing_revision.is_none(),
        "Esc must clear viewing_revision"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_clears_viewing_revision_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.git_mode = true;
    app.viewing_revision = Some("abc1234".to_string());
    app.handle_key(key(KeyCode::Esc));
    assert!(
        app.viewing_revision.is_none(),
        "Esc in git mode must clear viewing_revision"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn f_key_scopes_content_search_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = true;
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('f')));
    assert!(
        app.search.as_ref().unwrap().scoped,
        "content search must be scoped when git mode is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn f_key_not_scoped_outside_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('f')));
    assert!(
        !app.search.as_ref().unwrap().scoped,
        "content search must not be scoped outside git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn r_key_clears_viewing_revision() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.viewing_revision = Some("abc1234".to_string());
    app.handle_key(key(KeyCode::Char('r')));
    assert!(
        app.viewing_revision.is_none(),
        "r (reload) must clear viewing_revision"
    );
    fs::remove_dir_all(&root).ok();
}

// -- PageUp/PageDown content navigation -------------------------------------

#[test]
fn content_page_down_moves_active_line_by_page_rows() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    let rows = app.page_rows();
    assert!(rows > 1, "page_rows must be > 1 for a meaningful test");
    app.active_line = 0;
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(
        app.active_line, rows,
        "PageDown must move active_line by page_rows()"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_page_up_moves_active_line_by_page_rows() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    let rows = app.page_rows();
    assert!(rows > 1, "page_rows must be > 1 for a meaningful test");
    app.active_line = rows * 2;
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(
        app.active_line, rows,
        "PageUp must move active_line up by page_rows()"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_page_down_clamps_at_last_display_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    let last = app.display_line_count().saturating_sub(1);
    app.active_line = last.saturating_sub(1);
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(
        app.active_line, last,
        "PageDown must clamp active_line to the last display line"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_page_up_clamps_at_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    app.active_line = 1;
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(app.active_line, 0, "PageUp must clamp active_line to 0");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_page_down_in_diff_only_scrolls() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.content_scroll = 5;
    app.active_line = 42;
    let scroll_before = app.content_scroll;
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(
        app.active_line, 42,
        "PageDown in diff mode must not change active_line"
    );
    assert!(
        app.content_scroll > scroll_before,
        "PageDown in diff mode must scroll the viewport forward"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_page_up_in_diff_only_scrolls() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.is_diff = true;
    app.content_scroll = 50;
    app.active_line = 42;
    let scroll_before = app.content_scroll;
    app.handle_key(key(KeyCode::PageUp));
    assert_eq!(
        app.active_line, 42,
        "PageUp in diff mode must not change active_line"
    );
    assert!(
        app.content_scroll < scroll_before,
        "PageUp in diff mode must scroll the viewport back"
    );
    fs::remove_dir_all(&root).ok();
}

// -- command palette ranking -------------------------------------------------

#[test]
fn ctrl_p_opens_palette_with_ranked_order_from_usage() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Pre-load usage so help (index 0) is the last-used command.
    app.command_usage.record("help");
    app.handle_key(KeyEvent::new(
        KeyCode::Char('p'),
        crossterm::event::KeyModifiers::CONTROL,
    ));
    let palette = app
        .command_palette
        .as_ref()
        .expect("Ctrl+P must open command_palette");
    assert!(
        palette.base_pinned >= 1,
        "palette must have at least one pinned entry when usage stats are set"
    );
    assert_eq!(
        palette.base_order[0], 0,
        "help (index 0) must be first in base_order as the last-used command"
    );
    fs::remove_dir_all(&root).ok();
}

// -- find_files (ctrl+f) ----------------------------------------------------

fn ctrl_f() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)
}

#[test]
fn ctrl_f_opens_file_picker_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(ctrl_f());
    assert!(
        app.search.is_some(),
        "ctrl+f must open the search picker from Tree focus"
    );
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        SearchMode::Files,
        "ctrl+f must open in Files mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ctrl_f_opens_file_picker_when_content_focused_with_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(ctrl_f());
    assert!(
        app.search.is_some(),
        "ctrl+f must open the search picker from Content focus with an open file"
    );
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        SearchMode::Files,
        "ctrl+f must open in Files mode (not in-file search)"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ctrl_f_opens_file_picker_when_content_focused_no_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(ctrl_f());
    assert!(
        app.search.is_some(),
        "ctrl+f must open the search picker from Content focus without a file"
    );
    fs::remove_dir_all(&root).ok();
}

// -- search_files (/) unchanged ----------------------------------------------

#[test]
fn slash_opens_tree_filter_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.tree_filter.is_some(),
        "/ must open tree filter from Tree focus"
    );
    assert!(app.search.is_none(), "/ must not open the search picker");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn slash_opens_in_file_search_when_content_focused_with_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.in_file_search.is_some(),
        "/ must open in-file search from Content focus with open file"
    );
    assert!(app.search.is_none(), "/ must not open the search picker");
    fs::remove_dir_all(&root).ok();
}

fn tree_with_nested() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_normal_nested_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn tree_collapse_on_child_navigates_to_parent_dir() {
    let root = tree_with_nested();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.expanded.insert(root.join("sub"));
    app.rebuild(true);
    let child_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub").join("c.txt"))
        .expect("sub/c.txt must be visible after expand");
    app.tree_selected = child_idx;
    app.handle_key(key(KeyCode::Left));
    assert_eq!(
        app.nodes[app.tree_selected].path,
        root.join("sub"),
        "Left on nested child must jump to parent dir"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn slash_opens_file_picker_when_content_focused_no_file() {
    // Use an empty directory so App::new() has no file to open, leaving
    // current_file = None.
    let dir = std::env::temp_dir().join(format!(
        "tv_slash_empty_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    let mut app = app_for(&dir);
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.search.is_some(),
        "/ must fall back to file picker from Content focus without a file"
    );
    fs::remove_dir_all(&dir).ok();
}
