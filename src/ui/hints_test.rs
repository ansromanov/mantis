use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::*;
use crate::config::Config;

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mantis_hints_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn actions(app: &App) -> Vec<&'static str> {
    hint_items(app).into_iter().map(|h| h.action).collect()
}

#[test]
fn tree_hints_lead_with_open_and_end_with_help_and_more() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let hints = hint_items(&app);
    assert_eq!(hints[0].action, "tree_expand");
    assert_eq!(hints[0].key, "Enter");
    let tail: Vec<_> = hints.iter().rev().take(2).map(|h| h.action).collect();
    assert_eq!(tail, vec!["context_menu", "help"]);
    fs::remove_dir_all(root).ok();
}

#[test]
fn content_hints_follow_the_open_file_and_skip_inapplicable_actions() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    let list = actions(&app);
    assert!(list.contains(&"goto_line"));
    assert!(list.contains(&"toggle_wrap"));
    assert!(!list.contains(&"diff_hunk_next"), "no diff is shown");
    assert!(
        !list.contains(&"toggle_raw_markdown"),
        "not a markdown file"
    );
    // Panel suffixes are stripped from keys shown in the row.
    assert!(hint_items(&app)
        .iter()
        .all(|h| !h.key.contains("(content)")));
    fs::remove_dir_all(root).ok();
}

#[test]
fn hints_use_the_live_keymap() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.keys_mut_for_tests().toggle_wrap = crate::config::bind(&["content:w"]);
    let wrap = hint_items(&app)
        .into_iter()
        .find(|h| h.action == "toggle_wrap")
        .unwrap();
    assert_eq!(wrap.key, "w");
    fs::remove_dir_all(root).ok();
}

#[test]
fn the_row_is_empty_while_a_popup_is_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_help = true;
    assert!(hint_items(&app).is_empty());
    fs::remove_dir_all(root).ok();
}

#[test]
fn clicking_a_hint_replays_its_key() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let mut terminal = Terminal::new(TestBackend::new(120, 1)).unwrap();
    let mut targets = Vec::new();
    terminal
        .draw(|f| targets = draw_hints(f, &app, f.area()))
        .unwrap();
    app.geo.hint_targets = targets.clone();
    let (rect, _) = targets
        .iter()
        .find(|(_, action)| *action == "help")
        .copied()
        .unwrap();
    app.handle_mouse(crossterm::event::MouseEvent {
        kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: crossterm::event::KeyModifiers::empty(),
    });
    assert!(app.show_help, "clicking the help hint presses its key");
    fs::remove_dir_all(root).ok();
}

#[test]
fn more_hint_shows_a_key_every_keyboard_has() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let more = hint_items(&app)
        .into_iter()
        .find(|h| h.action == "context_menu")
        .unwrap();
    assert_eq!(more.key, "Shift+F10");
    fs::remove_dir_all(root).ok();
}
