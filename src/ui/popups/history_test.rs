use crate::app::App;
use crate::config::Config;
use crate::git::Commit;
use crate::search::HistoryState;
use crate::ui::popups::draw_history;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn make_app(root: &std::path::Path) -> App {
    let cfg = Config {
        git_status: false,
        ..Config::default()
    };
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn draw_history_none_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.history = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_history(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_history_some_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.history = Some(HistoryState::new(
        dir.path().join("f"),
        vec![Commit {
            hash: "abc123".into(),
            short: "abc".into(),
            date: "2024-01-01".into(),
            subject: "test commit".into(),
        }],
    ));
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_history(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}
