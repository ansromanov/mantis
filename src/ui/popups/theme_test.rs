use crate::app::App;
use crate::config::Config;
use crate::search::ThemePicker;
use crate::ui::popups::draw_theme;
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
fn draw_theme_none_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.theme_picker = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_theme(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_theme_some_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.theme_picker = Some(ThemePicker::default());
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_theme(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

// Modified for test requirements
