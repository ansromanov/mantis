//! Tests for the virtual-file and inline-fallback render arms (see
//! `draw_text.rs`). Both are exercised through `draw_content` since the
//! render functions take the app + layout and are not meant to be called in
//! isolation.

use ratatui::backend::TestBackend;
use ratatui::style::Style;

use crate::ui::content::draw_content;

fn render_app() -> (crate::app::App, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
    let app = crate::app::App::new(
        dir.path().to_path_buf(),
        crate::config::Config::default(),
        None,
        None,
    )
    .expect("App::new");
    (app, dir)
}

fn render<F: FnOnce(&mut crate::app::App)>(app: &mut crate::app::App, f: F) -> String {
    f(app);
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn render_virtual_file_shows_line_numbers_and_text() {
    let (mut app, dir) = render_app();
    let path = dir.path().join("f.txt");
    app.open_file(&path);
    assert!(
        app.virtual_file.is_some(),
        "plain file should mmap as virtual"
    );

    let screen = render(&mut app, |_| {});
    assert!(
        screen.contains("line1"),
        "content should render; got: {screen:?}"
    );
    assert!(screen.contains('1'), "line-number gutter should render");
}

#[test]
fn render_inline_fallback_renders_loaded_content() {
    let (mut app, _dir) = render_app();
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["alpha".to_string(), "beta".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "alpha".to_string())],
            vec![(Style::default(), "beta".to_string())],
        ];
    });
    assert!(screen.contains("alpha"), "got: {screen:?}");
    assert!(screen.contains("beta"), "got: {screen:?}");
}

#[test]
fn render_inline_fallback_empty_does_not_panic() {
    let (mut app, _dir) = render_app();
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = Vec::new();
        app.highlighted = Vec::new();
    });
    assert!(!screen.is_empty());
}
