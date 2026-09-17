use super::*;
use crate::app::App;
use crate::config::Config;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;

fn test_app() -> App {
    App::new(PathBuf::from("."), Config::default(), None, None).unwrap()
}

#[test]
fn draw_diff_info_bar_zero_area_noop() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let app = test_app();
    terminal
        .draw(|f| {
            draw_diff_info_bar(f, &app, Rect::default());
        })
        .unwrap();
}

#[test]
fn draw_diff_info_bar_renders_mode_and_hunks() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = test_app();
    app.diff_mode = crate::app::DiffMode::Unstaged;
    app.content = vec![
        "@@ -1,3 +1,4 @@".to_string(),
        "+added line".to_string(),
        "@@ -10,3 +11,4 @@".to_string(),
    ];
    let area = Rect::new(0, 0, 80, 1);
    terminal
        .draw(|f| {
            draw_diff_info_bar(f, &app, area);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row_str: String = (0..80)
        .map(|x| buffer[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row_str.contains("Diff: unstaged"),
        "expected row to contain mode: {row_str}"
    );
    assert!(
        row_str.contains("2 hunks"),
        "expected row to contain hunk count: {row_str}"
    );
    assert!(
        row_str.contains("[unified]"),
        "expected row to show layout: {row_str}"
    );
}

#[test]
fn draw_diff_info_bar_side_by_side_layout_label() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut app = test_app();
    app.diff_side_by_side = true;
    let area = Rect::new(0, 0, 80, 1);
    terminal
        .draw(|f| {
            draw_diff_info_bar(f, &app, area);
        })
        .unwrap();
    let buffer = terminal.backend().buffer();
    let row_str: String = (0..80)
        .map(|x| buffer[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row_str.contains("[side-by-side]"),
        "expected side-by-side: {row_str}"
    );
}
