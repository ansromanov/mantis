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
    app.bug_report = Some(BugReportState::new());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_bug_report(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_bug_report_with_text_does_not_panic() {
    let mut app = test_app();
    let mut state = BugReportState::new();
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
