use super::*;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

use crate::app::App;
use crate::config::Config;
use crate::search::BugReportState;

fn test_app() -> App {
    App::new(std::env::temp_dir(), Config::default(), None, None).unwrap()
}

#[test]
fn draw_bug_report_none_does_not_panic() {
    let mut app = test_app();
    app.bug_report = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_bug_report_empty_does_not_panic() {
    let mut app = test_app();
    app.bug_report = Some(BugReportState::default());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_bug_report_with_text_does_not_panic() {
    let mut app = test_app();
    let mut state = BugReportState::default();
    state.insert_char('H');
    state.insert_char('i');
    state.insert_newline();
    state.insert_char('T');
    state.insert_char('e');
    state.insert_char('s');
    state.insert_char('t');
    app.bug_report = Some(state);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_bug_report_populates_areas() {
    let mut app = test_app();
    app.bug_report = Some(BugReportState::default());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();

    assert!(app.bug_report_area.width > 0);
    assert!(app.bug_report_area.height > 0);
    assert!(app.bug_report_preview_area.width > 0);
    assert!(app.bug_report_preview_area.height > 0);
}

#[test]
fn draw_bug_report_long_line_wraps_without_panic() {
    let mut app = test_app();
    let mut state = BugReportState::default();
    // Insert a line longer than the description edit width
    let long_line = "This is a very long line that should wrap to multiple visual rows in the bug report description editor field without causing any panic or visual glitch.";
    for c in long_line.chars() {
        state.insert_char(c);
    }
    app.bug_report = Some(state);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_bug_report_long_line_scrolled_does_not_panic() {
    let mut app = test_app();
    let mut state = BugReportState::default();
    // Add many long lines so scrolling is required
    for _ in 0..20 {
        let line = "A".repeat(40);
        for c in line.chars() {
            state.insert_char(c);
        }
        state.insert_newline();
    }
    // Move cursor to the end and scroll down
    state.cursor_row = state.text.len() - 1;
    app.bug_report = Some(state);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}
