use crate::app::App;
use crate::config::Config;
use crate::search::{FilterBarState, InFileSearch};
use crate::ui::popups::{draw_filter_bar, draw_in_file_search};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn make_app(root: &std::path::Path) -> App {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        ..Config::default()
    };
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn draw_in_file_search_none_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.in_file_search = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_in_file_search_some_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.in_file_search = Some(InFileSearch::new());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_in_file_search_some_renders_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut s = InFileSearch::new();
    s.case_sensitive = true;
    app.in_file_search = Some(s);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut rendered_text = String::new();
    for y in 0..24 {
        for x in 0..80 {
            rendered_text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        rendered_text.push('\n');
    }
    assert!(rendered_text.contains("[Aa]"));
}

#[test]
fn draw_filter_bar_renders_query_cursor_and_hidden_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.content = vec![
        "error one".to_string(),
        "info two".to_string(),
        "error three".to_string(),
    ];
    app.filter_display_map = vec![0, 2];
    let mut s = FilterBarState::new();
    s.query.push_str("error");
    app.filter_bar = Some(s);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_filter_bar(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let mut rendered_text = String::new();
    for y in 0..24 {
        for x in 0..80 {
            rendered_text.push(buffer[(x, y)].symbol().chars().next().unwrap_or(' '));
        }
        rendered_text.push('\n');
    }
    assert!(rendered_text.contains("Filter: error"));
    assert!(rendered_text.contains('\u{2588}'));
    assert!(rendered_text.contains("hidden: 1"));
}
// touched for log follow mode
