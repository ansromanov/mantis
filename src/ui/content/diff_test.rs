use ratatui::backend::TestBackend;
use ratatui::style::{Color, Style};

use crate::ui::content::diff::emphasize;
use crate::ui::content::draw_content;

// ── emphasize ──────────────────────────────────────────────────────────

#[test]
fn emphasize_no_ranges_returns_full_text() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("hello", &[], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello");
}

#[test]
fn emphasize_middle_range_splits_correctly() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("hello world", &[(6, 11)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello ");
    assert_eq!(result[0].style, base);
    assert_eq!(result[1].content, "world");
    assert_eq!(result[1].style, emph);
}

#[test]
fn emphasize_range_at_start() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Blue);
    let result = emphasize("abcdef", &[(0, 3)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "abc");
    assert_eq!(result[0].style, emph);
    assert_eq!(result[1].content, "def");
    assert_eq!(result[1].style, base);
}

#[test]
fn emphasize_range_at_end() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Green);
    let result = emphasize("abcdef", &[(3, 6)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "abc");
    assert_eq!(result[0].style, base);
    assert_eq!(result[1].content, "def");
    assert_eq!(result[1].style, emph);
}

#[test]
fn emphasize_full_range() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Yellow);
    let result = emphasize("full", &[(0, 4)], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "full");
    assert_eq!(result[0].style, emph);
}

#[test]
fn emphasize_multiple_disjoint_ranges() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Magenta);
    let result = emphasize("abcdefghi", &[(1, 3), (5, 8)], base, emph);
    let joined: String = result.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(joined, "abcdefghi");
    let emphasized: String = result
        .iter()
        .filter(|s| s.style.bg == Some(Color::Magenta))
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(emphasized, "bcfgh");
}

#[test]
fn emphasize_empty_text() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("", &[(0, 0)], base, emph);
    assert_eq!(result.len(), 0);
}

#[test]
fn emphasize_range_out_of_bounds_clamps() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Cyan);
    let result = emphasize("hi", &[(10, 20)], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hi");
    assert_eq!(result[0].style, base);
}

// ── draw_side_by_side_diff ──────────────────────────────────────────────

fn render_app() -> (crate::app::App, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let app = crate::app::App::new(
        dir.path().to_path_buf(),
        crate::config::Config::default(),
        None,
        None,
    )
    .expect("App::new");
    (app, dir)
}

fn render_diff(
    app: &mut crate::app::App,
    rows: Vec<crate::diff::DiffRow>,
) -> ratatui::buffer::Buffer {
    app.current_file = None;
    app.virtual_file = None;
    app.is_diff = true;
    app.diff_side_by_side = true;
    app.diff_rows = rows;
    let backend = TestBackend::new(100, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn buffer_text(buffer: &ratatui::buffer::Buffer) -> String {
    buffer
        .content()
        .chunks(buffer.area.width as usize)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn side_by_side_diff_renders_both_columns() {
    let (mut app, _dir) = render_app();
    let lines: Vec<String> = ["@@ -1,2 +1,2 @@", "-old line", "+new line", " context"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows = crate::diff::parse_side_by_side(&lines);
    let buffer = render_diff(&mut app, rows);
    let text = buffer_text(&buffer);
    assert!(text.contains("old line"), "left column missing: {text}");
    assert!(text.contains("new line"), "right column missing: {text}");
    assert!(text.contains("context"), "context row missing: {text}");
}

#[test]
fn side_by_side_diff_shows_hunk_header() {
    let (mut app, _dir) = render_app();
    let lines: Vec<String> = ["@@ -5,1 +5,1 @@", "-removed", "+added"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows = crate::diff::parse_side_by_side(&lines);
    let buffer = render_diff(&mut app, rows);
    let text = buffer_text(&buffer);
    assert!(
        text.contains("@@ -5,1 +5,1 @@"),
        "hunk header missing: {text}"
    );
}

#[test]
fn side_by_side_diff_colors_added_and_removed() {
    let (mut app, _dir) = render_app();
    let theme = app.theme.clone();
    let lines: Vec<String> = ["@@ -1 +1 @@", "-old", "+new"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let rows = crate::diff::parse_side_by_side(&lines);
    let buffer = render_diff(&mut app, rows);
    let has_del = buffer.content().iter().any(|c| c.fg == theme.diff_del);
    let has_add = buffer.content().iter().any(|c| c.fg == theme.diff_add);
    assert!(has_del, "removed line should use diff_del color");
    assert!(has_add, "added line should use diff_add color");
}

#[test]
fn side_by_side_diff_scroll_within_bounds_does_not_panic() {
    let (mut app, _dir) = render_app();
    let mut lines = vec!["@@ -1,50 +1,50 @@".to_string()];
    lines.extend((0..50).map(|i| format!(" context line {i}")));
    let rows = crate::diff::parse_side_by_side(&lines);
    app.content_scroll = 10;
    let _buffer = render_diff(&mut app, rows);
}
