use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::{App, Focus};
use crate::config::Config;
use crate::pager::PagerContent;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_app_pager_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn open_pager_content_plain_text_sets_content_and_focus() {
    let root = temp_dir();
    let mut app = app_for(&root);
    let parsed = PagerContent {
        content: vec!["hello".to_string(), "world".to_string()],
        is_diff: false,
    };
    app.open_pager_content(parsed, None);
    assert_eq!(app.content, vec!["hello", "world"]);
    assert!(!app.is_diff);
    assert!(app.diff_rows.is_empty());
    assert_eq!(app.focus, Focus::Content);
    assert_eq!(app.tree_width, 0);
    assert!(app.current_file.is_none());
    assert_eq!(app.content_title.as_deref(), Some(" <stdin> "));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_pager_content_uses_explicit_language() {
    let root = temp_dir();
    let mut app = app_for(&root);
    let parsed = PagerContent {
        content: vec!["fn main() {".to_string()],
        is_diff: false,
    };
    app.open_pager_content(parsed, Some("rust".to_string()));
    assert_eq!(app.current_syntax.as_deref(), Some("Rust"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_pager_content_diff_populates_diff_rows() {
    let root = temp_dir();
    let mut app = app_for(&root);
    let parsed = PagerContent {
        content: vec![
            "diff --git a/f b/f".to_string(),
            "--- a/f".to_string(),
            "+++ b/f".to_string(),
            "@@ -1 +1 @@".to_string(),
            "-old".to_string(),
            "+new".to_string(),
        ],
        is_diff: true,
    };
    app.open_pager_content(parsed, None);
    assert!(app.is_diff);
    assert!(!app.diff_rows.is_empty());
    assert_eq!(app.content_title.as_deref(), Some(" <stdin> — diff "));
    // `GIT_PAGER=mantis` should render side-by-side out of the box.
    assert!(app.diff_side_by_side);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_pager_content_resets_scroll_and_selection() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.content_hscroll = 5;
    app.active_line = 3;
    app.show_line_blame = true;
    let parsed = PagerContent {
        content: vec!["a".to_string(), "b".to_string(), "c".to_string()],
        is_diff: false,
    };
    app.open_pager_content(parsed, None);
    assert_eq!(app.content_scroll, 0);
    assert_eq!(app.content_hscroll, 0);
    assert_eq!(app.active_line, 0);
    assert!(!app.show_line_blame);
    assert!(app.selection.is_none());
    fs::remove_dir_all(&root).ok();
}
