use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;

use crate::app::App;
use crate::config::Config;

use super::draw_content_scrollbar;

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn render(app: &App, inner_w: usize, inner_h: usize) -> ratatui::buffer::Buffer {
    let backend = TestBackend::new(inner_w as u16 + 2, inner_h as u16 + 2);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_content_scrollbar(f, app, 1, 1, inner_w, inner_h))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn has_thumb(buffer: &ratatui::buffer::Buffer) -> bool {
    buffer.content().iter().any(|c| c.symbol() == "█")
}

#[test]
fn scrollbar_hidden_when_disabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());
    app.show_scrollbar = false;
    app.content = (0..100).map(|i| format!("line {i}")).collect();
    app.content_scrolled_at = Instant::now();
    let buffer = render(&app, 10, 10);
    assert!(!has_thumb(&buffer), "scrollbar must not draw when disabled");
}

#[test]
fn scrollbar_hidden_when_content_fits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());
    app.show_scrollbar = true;
    app.content = vec!["only line".to_string()];
    app.content_scrolled_at = Instant::now();
    let buffer = render(&app, 10, 10);
    assert!(
        !has_thumb(&buffer),
        "scrollbar must not draw when total lines fit within the view"
    );
}

#[test]
fn scrollbar_hidden_after_fade_window_elapses() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());
    app.show_scrollbar = true;
    app.content = (0..100).map(|i| format!("line {i}")).collect();
    app.content_scrolled_at = Instant::now() - Duration::from_millis(3000);
    let buffer = render(&app, 10, 10);
    assert!(
        !has_thumb(&buffer),
        "scrollbar must fade out after SCROLLBAR_FADE elapses"
    );
}

#[test]
fn scrollbar_visible_when_recently_scrolled_and_overflowing() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());
    app.show_scrollbar = true;
    app.content = (0..100).map(|i| format!("line {i}")).collect();
    app.content_scrolled_at = Instant::now();
    let buffer = render(&app, 10, 10);
    assert!(
        has_thumb(&buffer),
        "scrollbar thumb should be visible right after scrolling"
    );
}

#[test]
fn scrollbar_thumb_moves_toward_bottom_as_scroll_increases() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());
    app.show_scrollbar = true;
    app.content = (0..1000).map(|i| format!("line {i}")).collect();
    app.content_scrolled_at = Instant::now();

    app.content_scroll = 0;
    let top_buffer = render(&app, 10, 10);
    let top_thumb_row = (0..10)
        .find(|&y| top_buffer[(10u16, y + 1)].symbol() == "█")
        .expect("thumb should render near the top");

    app.content_scroll = 990;
    let bottom_buffer = render(&app, 10, 10);
    let bottom_thumb_row = (0..10)
        .find(|&y| bottom_buffer[(10u16, y + 1)].symbol() == "█")
        .expect("thumb should render near the bottom");

    assert!(
        bottom_thumb_row > top_thumb_row,
        "thumb should move down as content_scroll increases: top={top_thumb_row} bottom={bottom_thumb_row}"
    );
}
