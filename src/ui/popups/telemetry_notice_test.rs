use super::*;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

use crate::app::App;
use crate::config::Config;

fn test_app() -> App {
    App::new(std::env::temp_dir(), Config::default(), None, None).unwrap()
}

#[test]
fn draw_telemetry_notice_does_not_panic() {
    let mut app = test_app();
    app.show_telemetry_notice = true;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_telemetry_notice(f, &app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_telemetry_notice_false_does_not_panic() {
    let mut app = test_app();
    app.show_telemetry_notice = false;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_telemetry_notice(f, &app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}
