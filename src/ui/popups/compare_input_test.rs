use ratatui::{backend::TestBackend, layout::Rect, Terminal};

use crate::app::App;
use crate::config::Config;
use crate::search::CompareModeInput;
use crate::ui::popups::draw_compare_input;

fn app() -> App {
    App::new(std::env::temp_dir(), Config::default(), None, None).unwrap()
}

#[test]
fn draw_compare_input_none_does_not_panic() {
    let mut app = app();
    app.compare_input = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_compare_input(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_compare_input_empty_does_not_panic() {
    let mut app = app();
    app.compare_input = Some(CompareModeInput::new());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_compare_input(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_compare_input_with_query_does_not_panic() {
    let mut app = app();
    let mut state = CompareModeInput::new();
    state.push('H');
    state.push('E');
    state.push('A');
    state.push('D');
    state.push('~');
    state.push('3');
    app.compare_input = Some(state);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_compare_input(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}
