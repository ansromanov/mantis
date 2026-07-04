// Tests for the plugin-content branches of the content-source queries:
// `line_count` and `line_text` must read from `plugin_content` /
// `plugin_content_text` when the current file has plugin-provided content.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::style::Style;

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_root() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_cquery_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("doc.md"), "placeholder\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn seed_plugin(app: &mut App, path: PathBuf, lines: &[&str]) {
    let rendered: Vec<Vec<(Style, String)>> = lines
        .iter()
        .map(|l| vec![(Style::default(), l.to_string())])
        .collect();
    let text: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
    app.plugin_content_text.insert(path.clone(), text);
    app.plugin_content.insert(path.clone(), rendered);
    app.current_file = Some(path);
}

#[test]
fn line_count_reads_plugin_content() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["a", "b", "c"]);
    assert_eq!(app.line_count(), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_text_reads_plugin_content_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    seed_plugin(&mut app, path, &["first", "second"]);
    assert_eq!(app.line_text(0), Some("first"));
    assert_eq!(app.line_text(1), Some("second"));
    assert_eq!(app.line_text(2), None);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_count_ignores_plugin_content_for_other_file() {
    let root = temp_root();
    let mut app = app_for(&root);
    // Plugin content keyed by a different path must not affect the open file.
    let other = root.join("other.md");
    let rendered: Vec<Vec<(Style, String)>> = (0..5)
        .map(|i| vec![(Style::default(), format!("l{i}"))])
        .collect();
    app.plugin_content.insert(other.clone(), rendered);
    app.plugin_content_text
        .insert(other, (0..5).map(|i| format!("l{i}")).collect());
    app.current_file = Some(root.join("doc.md"));
    // Falls through to the normal content source, not the 5-line plugin entry
    // keyed by `other.md`.
    assert_ne!(app.line_count(), 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_count_md_file_uses_virtual_file_not_builtin_markdown() {
    let root = temp_root();
    let mut app = app_for(&root);
    let path = root.join("doc.md");
    fs::write(&path, "line1\nline2\nline3\n").unwrap();
    app.open_file(&path);
    // Without the built-in markdown renderer, .md files fall through to
    // VirtualFile, so line_count reflects the raw file.
    assert_eq!(app.line_count(), 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn highlight_lines_uses_current_syntax_not_a_path_arg() {
    // highlight_lines was changed to route through `self.current_syntax`
    // (resolved once at file-open time) instead of taking a path, so
    // repeated scroll redraws don't re-open the file to sniff its syntax.
    let root = temp_root();
    let mut app = app_for(&root);
    app.current_syntax = Some("Rust".to_string());
    let result = app.highlight_lines(&["fn main() {"]);
    assert!(
        result[0].len() > 1,
        "Rust syntax should produce multiple styled spans, got {}",
        result[0].len()
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn highlight_lines_none_current_syntax_is_plain_text() {
    let root = temp_root();
    let mut app = app_for(&root);
    app.current_syntax = None;
    let result = app.highlight_lines(&["fn main() {"]);
    assert_eq!(
        result[0].len(),
        1,
        "no current_syntax should fall back to a single plain-text span"
    );
    fs::remove_dir_all(&root).ok();
}
