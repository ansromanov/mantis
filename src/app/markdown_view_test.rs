use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_markdown_view_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn long_markdown() -> String {
    let mut s = String::from("# Title\n\n");
    for i in 0..200 {
        s.push_str(&format!("Paragraph number {i}.\n\n"));
    }
    s
}

#[test]
fn markdown_is_rendered_synchronously_on_open() {
    let root = temp_dir();
    let md = root.join("doc.md");
    fs::write(&md, "# Title\n\nbody text\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&md);
    assert!(
        app.rendered_active,
        "rendered view must be active as soon as the file is open, with no plugin round trip"
    );
    let first = app.line_text(0).unwrap_or_default();
    assert!(first.contains("Title"), "got {first:?}");
    assert!(
        !first.contains('#'),
        "heading marker should be rendered away: {first:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn raw_toggle_shows_source_and_back_again() {
    let root = temp_dir();
    let md = root.join("doc.md");
    fs::write(&md, "# Title\n\nbody text\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&md);

    app.toggle_raw_markdown();
    assert!(app.show_raw_markdown);
    assert!(!app.rendered_active);
    assert_eq!(app.line_text(0), Some("# Title"));

    app.toggle_raw_markdown();
    assert!(!app.show_raw_markdown);
    assert!(app.rendered_active);
    assert!(!app.line_text(0).unwrap_or_default().contains('#'));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn same_file_reload_keeps_rendered_view_and_scroll() {
    let root = temp_dir();
    let md = root.join("doc.md");
    fs::write(&md, long_markdown()).unwrap();
    let mut app = app_for(&root);
    app.open_file(&md);
    app.set_content_scroll(20);
    let before = app.content_scroll;
    assert!(before > 0, "fixture should be long enough to scroll");

    app.reload_content();

    assert!(app.rendered_active, "reload must not drop the view");
    assert_eq!(app.content_scroll, before, "reload must not reset scroll");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn oversized_markdown_falls_back_to_raw() {
    let root = temp_dir();
    let md = root.join("big.md");
    let line = "x".repeat(1000) + "\n";
    fs::write(&md, line.repeat(2200)).unwrap(); // ~2.2 MB
    let mut app = app_for(&root);
    app.open_file(&md);
    assert!(!app.rendered_active);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn non_markdown_files_are_not_rendered() {
    let root = temp_dir();
    let txt = root.join("notes.txt");
    fs::write(&txt, "# not a heading\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&txt);
    assert!(!app.rendered_active);
    fs::remove_dir_all(&root).ok();
}
