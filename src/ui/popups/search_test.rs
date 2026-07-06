use crate::app::App;
use crate::config::Config;
use crate::search::SearchState;
use crate::ui::popups::draw_search;
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
fn draw_search_none_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.search = None;
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_search_shows_scoped_title() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut files = std::collections::HashSet::new();
    files.insert(dir.path().join("changed.txt"));
    std::fs::write(dir.path().join("changed.txt"), "").unwrap();
    app.search = Some(SearchState::new(dir.path(), false, false, 2, Some(&files)));
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows: Vec<String> = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect();
    let joined = rows.join("\n");
    assert!(
        joined.contains("(changed files)"),
        "scoped search should show '(changed files)' in title"
    );
}

#[test]
fn draw_search_shows_unscoped_title() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    app.search = Some(SearchState::new(dir.path(), false, false, 2, None));
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows: Vec<String> = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect();
    let joined = rows.join("\n");
    assert!(
        !joined.contains("(changed files)"),
        "unscoped search should not show '(changed files)' in title"
    );
}

#[test]
fn draw_search_some_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.search = Some(SearchState::new(dir.path(), false, false, 2, None));
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_search_renders_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut s = SearchState::new(dir.path(), false, false, 2, None);
    s.case_sensitive = true;
    app.search = Some(s);
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let rows: Vec<String> = (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect();
    let joined = rows.join("\n");
    assert!(joined.contains("[Aa]"));
}

// Modified for test requirements
