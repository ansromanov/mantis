use super::*;
use crate::app::App;
use crate::config::Config;
use ratatui::backend::TestBackend;
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
fn draw_welcome_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_welcome = true;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_welcome(f, &mut app, f.area()))
        .unwrap();

    assert_eq!(app.welcome_area.width, 48); // 80 * 60% = 48
    assert_eq!(app.welcome_area.height, 21); // 30 * 70% = 21
}
