use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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

// -- navigation clears blame popup ------------------------------------------

#[test]
fn nav_up_clears_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 5;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('k')));
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn nav_down_clears_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.show_line_blame = true;
    app.handle_key(key(KeyCode::Char('j')));
    assert!(!app.show_line_blame);
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
    fs::remove_dir_all(&root).ok();
}
