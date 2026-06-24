// Tests for plugin-content geometry: selection extraction over styled spans and
// the line-number gutter being suppressed for plugin-rendered content.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::style::Style;

use crate::app::App;
use crate::config::Config;
use crate::selection::TextSelection;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_cpos_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("doc.md"), "placeholder\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn span(text: &str) -> Vec<(Style, String)> {
    vec![(Style::default(), text.to_string())]
}

/// Seed `plugin_content` (styled spans) and `plugin_content_text` for `path`.
fn seed_plugin(app: &mut App, path: PathBuf, lines: &[&str]) {
    let rendered: Vec<Vec<(Style, String)>> = lines.iter().map(|l| span(l)).collect();
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    app.plugin_content_text.insert(path.clone(), text);
    app.plugin_content.insert(path.clone(), rendered);
    app.current_file = Some(path);
}

#[test]
fn selection_text_single_line_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["hello world", "second line"]);
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 5),
    });
    assert_eq!(app.selection_text(), "hello");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_multi_line_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["hello world", "second line"]);
    // From col 6 of line 0 through col 6 of line 1.
    app.selection = Some(TextSelection {
        anchor: (0, 6),
        active: (1, 6),
    });
    assert_eq!(app.selection_text(), "world\nsecond");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_plugin_clamps_out_of_range_end() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["abc"]);
    // end_line past the buffer must clamp to the last line without panicking.
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (9, 99),
    });
    assert_eq!(app.selection_text(), "abc");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_prefix_width_zero_for_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["x", "y"]);
    app.show_line_numbers = true;
    assert_eq!(app.line_prefix_width(), 0);
    fs::remove_dir_all(&root).ok();
}
