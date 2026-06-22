use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::{App, Focus};
use crate::command_palette::COMMANDS;
use crate::config::Config;
use crate::search::CommandPalette;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_editor_key_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn dispatch_blame_line(app: &mut App) {
    let mut p = CommandPalette::default();
    for c in "Blame active".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    app.dispatch_command();
}

// -- blame_line action via command palette -----------------------------------

#[test]
fn editor_blame_line_action_toggles_show_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;

    assert!(!app.show_line_blame);
    dispatch_blame_line(&mut app);
    assert!(app.show_line_blame);

    dispatch_blame_line(&mut app);
    assert!(!app.show_line_blame);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn editor_blame_line_action_noop_when_is_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;

    dispatch_blame_line(&mut app);
    assert!(!app.show_line_blame);

    fs::remove_dir_all(&root).ok();
}

// -- blame_line present in COMMANDS -----------------------------------------

#[test]
fn commands_includes_blame_line_action() {
    assert!(
        COMMANDS.iter().any(|c| c.action_id == "blame_line"),
        "blame_line must be registered in COMMANDS"
    );
}

#[test]
fn commands_blame_line_has_expected_name() {
    let entry = COMMANDS.iter().find(|c| c.action_id == "blame_line").unwrap();
    assert_eq!(entry.name, "Blame active line");
}
