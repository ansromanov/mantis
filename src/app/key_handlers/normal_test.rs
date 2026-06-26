use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::app::{App, Focus};
use crate::config::Config;

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
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("flat view: only in git mode (Ctrl+G)")
    );
    assert!(!app.git_mode_flat);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_raw_markdown_noop_on_non_markdown() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    assert!(!app.is_markdown);
    app.handle_key(key(KeyCode::Char('M')));
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("raw toggle: not a markdown file")
    );
    assert!(!app.show_raw_markdown);
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
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT));
    assert!(app.status_message.is_some());
    app.handle_key(key(KeyCode::Char('j')));
    assert!(app.status_message.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_raw_markdown_status_clears_on_valid_keypress() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('M')));
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
    assert_eq!(cb.get_text().unwrap(), dir_path.display().to_string());
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
    assert_eq!(cb.get_text().unwrap(), rel);
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
    assert_eq!(
        cb.get_text().unwrap(),
        root.join("long.txt").display().to_string()
    );
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
    assert_eq!(cb.get_text().unwrap(), "long.txt");
    fs::remove_dir_all(&root).ok();
}
