use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::SearchMode;

#[cfg(unix)]
use crate::event_source::{AltKeys, CURRENT_ALT_KEYS};

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

fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
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

// -- Esc does not close bottom-bar blame (only full-file blame) --------------

#[test]
fn esc_does_not_close_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(
        app.show_line_blame,
        "Esc must not close single-line blame bar"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_closes_full_file_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_blame = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_blame, "Esc must close full-file blame pane");
    fs::remove_dir_all(&root).ok();
}

// -- blame pane navigation (full-file blame active, tree key routing) --------

#[test]
fn blame_pane_nav_up_moves_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Tree;
    app.show_blame = true;
    app.active_line = 5;
    app.handle_key(key(KeyCode::Char('k')));
    assert_eq!(app.active_line, 4, "nav up moves active_line in blame pane");
}

#[test]
fn blame_pane_nav_down_moves_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Tree;
    app.show_blame = true;
    app.active_line = 5;
    app.handle_key(key(KeyCode::Char('j')));
    assert_eq!(
        app.active_line, 6,
        "nav down moves active_line in blame pane"
    );
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
    app.handle_key(ctrl('g'));
    assert!(app.goto_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ctrl_g_opens_goto_line_not_git_mode() {
    // goto_line owns ctrl+g; git_mode_toggle lives on ctrl+d — the two must
    // never collide regardless of terminal capabilities.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    #[cfg(unix)]
    CURRENT_ALT_KEYS.with(|c| {
        c.set(AltKeys {
            shifted: None,
            base: None,
        })
    });
    app.handle_key(ctrl('g'));
    assert!(
        app.goto_line.is_some(),
        "goto_line dialog must open when ctrl+g is pressed"
    );
    assert!(!app.git_mode, "git_mode must NOT be toggled by ctrl+g");
    #[cfg(unix)]
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_keybinding_is_noop_with_tree_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(ctrl('g'));
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
    app.handle_key(ctrl('g'));
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
    // Descend to sub so app.root is root/sub
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("nested.txt"), "nested").unwrap();
    app.rebuild(true);
    let sub_idx = app.nodes.iter().position(|n| n.path == sub).unwrap();
    app.tree_selected = sub_idx;
    app.descend_to_selected();

    app.focus = Focus::Tree;
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == sub.join("nested.txt"))
        .expect("nested.txt");
    app.tree_selected = file_idx;
    app.handle_key(key(KeyCode::Backspace));
    assert_eq!(
        app.root, orig_root,
        "Backspace must call tree_up_dir and change root back to initial_root"
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
        Some("flat view: only in git mode (Ctrl+D)")
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
    // toggle_pretty_json ships unbound (palette-only); bind a key for the test.
    app.keys.toggle_pretty_json = crate::config::bind(&["J"]);
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
    app.handle_key(ctrl('b'));
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
    app.keys.toggle_pretty_json = crate::config::bind(&["J"]);
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
    app.handle_key(ctrl('b'));
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
    // toggle_diff_side_by_side ships unbound (palette-only); bind a key.
    app.keys.toggle_diff_side_by_side = crate::config::bind(&["D"]);
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
    // toggle_diff_staged ships unbound (palette-only); bind a key.
    app.keys.toggle_diff_staged = crate::config::bind(&["S"]);
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
    // toggle_wrap ships unbound (palette-only); bind a key for the test.
    app.keys.toggle_wrap = crate::config::bind(&["z"]);
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
    // toggle_line_numbers ships unbound (palette-only); bind a key.
    app.keys.toggle_line_numbers = crate::config::bind(&["L"]);
    app.handle_key(key(KeyCode::Char('L')));
    assert_eq!(app.show_line_numbers, !initial, "app field should toggle");
    assert_eq!(
        app.config.content.line_numbers, !initial,
        "config.content.line_numbers should persist the toggle"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_raw_markdown_not_active_shows_status() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;

    // By default, the plugin is not active. Pressing 'M' should show a status message.
    app.handle_key(key(KeyCode::Char('M')));
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .text
        .contains("not available"));

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
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(dir_path.display().to_string().as_str())
    );
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
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(rel.as_str())
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_file_from_content_still_works() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.copy_path_to_clipboard(false);
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(root.join("long.txt").display().to_string().as_str())
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
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("long.txt")
    );
    fs::remove_dir_all(&root).ok();
}

// -- copy line / copy file --------------------------------------------------

#[test]
fn copy_line_copies_active_line_text() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 2; // "line 3" (0-indexed, display coords)
    app.copy_line_or_selection();
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line 3"),
    );
    let sm = app.status_message.as_ref().expect("status must be set");
    assert_eq!(sm.text, "copied line");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_line_with_selection_copies_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    // Set a selection spanning "line 2\nline 3" (cols 0..6 on each line)
    app.selection = Some(crate::selection::TextSelection {
        anchor: (1, 0),
        active: (2, 6),
    });
    app.active_line = 2;
    app.copy_line_or_selection();
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line 2\nline 3"),
    );
    let sm = app.status_message.as_ref().expect("status must be set");
    assert_eq!(sm.text, "copied selection");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_line_empty_selection_still_copies_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;
    // Empty selection (anchor == active) should be treated as no selection.
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 0),
        active: (0, 0),
    });
    app.copy_line_or_selection();
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line 1"),
        "empty selection should fall back to copying the active line",
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_file_copies_entire_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.copy_file_content();
    // long.txt has 50 lines: "line 1" .. "line 50"
    let expected: String = (1..=50)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(expected.as_str()),
    );
    let sm = app.status_message.as_ref().expect("status must be set");
    assert_eq!(sm.text, "copied file");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_line_uses_display_to_physical_when_folded() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    use std::collections::HashSet;
    // Simulate a fold: region covering lines 2..4 (physical) is folded.
    app.fold_regions = vec![crate::fold::FoldRegion { start: 2, end: 4 }];
    let mut folded = HashSet::new();
    folded.insert(0); // region index 0 is folded
    app.fold_display_map = crate::fold::build_display_map(&app.fold_regions, &folded, 50);
    // After folding, display line 3 maps to physical line 5 (0-indexed).
    app.active_line = 3;
    app.copy_line_or_selection();
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line 6"), // physical line 5 (0-indexed) is "line 6"
        "copy_line must fold to the correct physical line",
    );
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
fn content_search_key_scopes_search_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = true;
    app.focus = Focus::Content;
    // search_content = ctrl+f; uppercase event must match case-insensitively.
    app.handle_key(ctrl('F'));
    assert!(
        app.search.as_ref().unwrap().scoped,
        "content search must be scoped when git mode is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_search_key_not_scoped_outside_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(ctrl('F'));
    assert!(
        !app.search.as_ref().unwrap().scoped,
        "content search must not be scoped outside git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn f_letter_opens_content_search_only_in_tree() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(key(KeyCode::Char('f')));
    assert!(app.search.is_some(), "tree:f opens content search");
    app.search = None;
    app.focus = Focus::Content;
    app.handle_key(key(KeyCode::Char('f')));
    assert!(
        app.search.is_none(),
        "bare f must not fire in the content pane (letter-free policy)"
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
    // command_palette = ctrl+p; uppercase event must match case-insensitively.
    app.handle_key(ctrl('P'));
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

#[test]
fn ctrl_p_computes_inapplicability_reasons_from_current_state() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // App::new auto-opens the first file (long.txt, not JSON), so
    // JSON-scoped commands must be marked inapplicable.
    app.handle_key(ctrl('P'));
    let palette = app
        .command_palette
        .as_ref()
        .expect("Ctrl+P must open command_palette");
    let json_idx = crate::command_palette::COMMANDS
        .iter()
        .position(|c| c.action_id == "toggle_pretty_json")
        .unwrap();
    let help_idx = crate::command_palette::COMMANDS
        .iter()
        .position(|c| c.action_id == "help")
        .unwrap();
    assert_eq!(
        palette.inapplicability_reasons[json_idx],
        Some("requires JSON file"),
        "toggle_pretty_json must be inapplicable when the open file isn't JSON"
    );
    assert_eq!(
        palette.inapplicability_reasons[help_idx], None,
        "help has no precondition and must always be applicable"
    );
    fs::remove_dir_all(&root).ok();
}

// -- find_files (ctrl+t) ----------------------------------------------------

#[test]
fn ctrl_t_opens_file_picker_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(ctrl('t'));
    assert!(
        app.search.is_some(),
        "ctrl+t must open the search picker from Tree focus"
    );
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        SearchMode::Files,
        "ctrl+t must open in Files mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ctrl_t_opens_file_picker_when_content_focused_with_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(ctrl('t'));
    assert!(
        app.search.is_some(),
        "ctrl+t must open the search picker from Content focus with an open file"
    );
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        SearchMode::Files,
        "ctrl+t must open in Files mode (not in-file search)"
    );
    fs::remove_dir_all(&root).ok();
}

// -- search_files (`/`, context-split) ----------------------------------------

#[test]
fn slash_opens_tree_filter_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.tree_filter.is_some(),
        "/ must open the tree filter from Tree focus"
    );
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
        "/ must open in-file search from Content focus with an open file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn slash_opens_file_picker_when_content_focused_no_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.search.is_some(),
        "/ must fall back to the search picker without a file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn slash_opens_in_file_search_for_pager_content_without_a_path() {
    // Piped stdin content (pager mode) has no backing path, but `content` is
    // populated, so it must still support in-file search rather than falling
    // back to the (irrelevant, since there's no tree) file picker.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_pager_content(
        crate::pager::PagerContent {
            content: vec!["one".to_string(), "two".to_string()],
            is_diff: false,
        },
        None,
    );
    app.handle_key(key(KeyCode::Char('/')));
    assert!(
        app.in_file_search.is_some(),
        "/ must open in-file search for loaded pager content"
    );
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
fn normal_key_o_triggers_open_external() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    assert!(app.status_message.is_none());
    app.handle_key(key(KeyCode::Char('o')));
    assert!(app.status_message.is_some());
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text.contains("not opening file"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_telemetry_check() {
    let root = temp_tree();
    let app = app_for(&root);
    assert!(!app.telemetry.is_enabled());
    fs::remove_dir_all(&root).ok();
}

// -- tree width grow/shrink (issue #665) ------------------------------------

#[test]
fn right_bracket_in_tree_mode_grows_tree_width() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_width = 50;
    app.handle_key(key(KeyCode::Char(']')));
    assert_eq!(app.tree_width, 52, "']' must increase tree_width by 2");
    assert_eq!(
        app.config.tree.width, 52,
        "config.tree.width must persist new tree_width"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn left_bracket_in_tree_mode_shrinks_tree_width() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_width = 50;
    app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(app.tree_width, 48, "'[' must decrease tree_width by 2");
    assert_eq!(
        app.config.tree.width, 48,
        "config.tree.width must persist new tree_width"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_width_grow_clamps_at_95() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_width = 94;
    app.handle_key(key(KeyCode::Char(']')));
    assert_eq!(app.tree_width, 95, "tree_width must clamp to 95 max");
    app.handle_key(key(KeyCode::Char(']')));
    assert_eq!(
        app.tree_width, 95,
        "tree_width must stay at 95 when already at max"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_width_shrink_clamps_at_5() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_width = 6;
    app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(app.tree_width, 5, "tree_width must clamp to 5 min");
    app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(
        app.tree_width, 5,
        "tree_width must stay at 5 when already at min"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_width_brackets_do_not_fire_in_content_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.tree_width = 50;
    app.handle_key(key(KeyCode::Char(']')));
    assert_eq!(
        app.tree_width, 50,
        "']' must not affect tree_width in content mode (tree-scoped binding)"
    );
    app.handle_key(key(KeyCode::Char('[')));
    assert_eq!(
        app.tree_width, 50,
        "'[' must not affect tree_width in content mode (tree-scoped binding)"
    );
    fs::remove_dir_all(&root).ok();
}
