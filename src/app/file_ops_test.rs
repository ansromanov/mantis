use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_file_ops_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

// -- push_recent ------------------------------------------------------------

#[test]
fn push_recent_adds_to_front() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(b.clone());
    assert_eq!(app.recent_ring[0], b);
    assert_eq!(app.recent_ring[1], a);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_deduplicates() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "a\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(a.clone());
    assert_eq!(app.recent_ring.len(), 1);
    assert_eq!(app.recent_ring[0], a);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_moves_existing_to_front() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(b.clone());
    assert_eq!(app.recent_ring[0], b);
    // Re-pushing a moves it to the front
    app.push_recent(a.clone());
    assert_eq!(app.recent_ring[0], a);
    assert_eq!(app.recent_ring[1], b);
    assert_eq!(app.recent_ring.len(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_caps_at_recent_files_count() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.config.recent_files_count = 3;
    for i in 0..5usize {
        let p = root.join(format!("{i}.txt"));
        fs::write(&p, "x\n").unwrap();
        app.push_recent(p);
    }
    assert_eq!(app.recent_ring.len(), 3);
    fs::remove_dir_all(&root).ok();
}

// -- open_recent_files ------------------------------------------------------

#[test]
fn open_recent_files_empty_ring_does_nothing() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.open_recent_files();
    assert!(app.recent_files.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_recent_files_excludes_current_file() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "a\n").unwrap();
    let mut app = app_for(&root);
    app.recent_ring = vec![a.clone()];
    app.current_file = Some(a);
    app.open_recent_files();
    // All entries are the current file, so overlay stays closed.
    assert!(app.recent_files.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_recent_files_opens_overlay_with_non_current_paths() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.recent_ring = vec![a.clone(), b.clone()];
    app.current_file = Some(a);
    app.open_recent_files();
    let state = app.recent_files.as_ref().unwrap();
    assert_eq!(state.paths.len(), 1);
    assert_eq!(state.paths[0], b);
    fs::remove_dir_all(&root).ok();
}

// -- active_line / show_line_blame reset on navigation ----------------------

#[test]
fn open_file_resets_active_line_and_blame_popup() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "line1\nline2\n").unwrap();
    fs::write(&b, "other\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&a);
    app.active_line = 5;
    app.show_line_blame = true;
    app.open_file(&b);
    assert_eq!(app.active_line, 0, "active_line must reset on file open");
    assert!(
        !app.show_line_blame,
        "show_line_blame must close on file open"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_sets_current_syntax_from_load() {
    let root = temp_dir();
    let f = root.join("main.rs");
    fs::write(&f, "fn main() {}\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&f);
    assert_eq!(
        app.current_syntax.as_deref(),
        Some("Rust"),
        "current_syntax should reflect detected language after file open"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_marks_session_dirty() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "line1\nline2\n").unwrap();
    let mut app = app_for(&root);
    app.session_dirty = false;
    app.open_file(&a);
    assert!(
        app.session_dirty,
        "opening a file must mark the session dirty so the new current_file persists"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_clears_current_syntax_for_unknown_type() {
    let root = temp_dir();
    let rs = root.join("main.rs");
    let unk = root.join("data.zzunknown");
    fs::write(&rs, "fn main() {}\n").unwrap();
    fs::write(&unk, "hello\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&rs);
    assert!(app.current_syntax.is_some(), "should detect Rust");
    app.open_file(&unk);
    assert_eq!(
        app.current_syntax, None,
        "current_syntax should be None for unknown extension"
    );
    fs::remove_dir_all(&root).ok();
}
