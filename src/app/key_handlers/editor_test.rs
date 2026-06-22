use crate::app::{App, Focus};
use crate::config::Config;
use crate::search::CommandPalette;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_editor_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn palette_with_query(query: &str) -> CommandPalette {
    let mut p = CommandPalette::default();
    for c in query.chars() {
        p.push(c);
    }
    p
}

#[test]
fn go_to_line_command_opens_dialog_when_content_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    // select the "Go to line" command and dispatch it
    app.command_palette = Some(palette_with_query("Go to line"));
    app.dispatch_command();
    assert!(app.goto_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn go_to_line_command_no_op_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.command_palette = Some(palette_with_query("Go to line"));
    app.dispatch_command();
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}
