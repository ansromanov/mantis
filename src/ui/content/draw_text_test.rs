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

#[test]
fn highlight_cache_populated_after_first_render() {
    let (mut app, dir) = render_app();
    app.open_file(&dir.path().join("f.txt"));
    assert!(app.virtual_file.is_some());
    assert!(
        app.content_highlight_cache.borrow().is_none(),
        "cache must be empty before any render"
    );
    render(&mut app, |_| {});
    assert!(
        app.content_highlight_cache.borrow().is_some(),
        "cache must be populated after rendering a virtual file"
    );
}

#[test]
fn highlight_cache_key_stable_on_identical_render() {
    let (mut app, dir) = render_app();
    app.open_file(&dir.path().join("f.txt"));
    render(&mut app, |_| {});
    let key_after_first = app
        .content_highlight_cache
        .borrow()
        .as_ref()
        .map(|(k, _)| (k.scroll, k.visible_end));
    render(&mut app, |_| {});
    let key_after_second = app
        .content_highlight_cache
        .borrow()
        .as_ref()
        .map(|(k, _)| (k.scroll, k.visible_end));
    assert_eq!(
        key_after_first, key_after_second,
        "cache key must be stable across identical renders"
    );
}

#[test]
fn blame_col_width_constant_is_37() {
    // draw_text.rs previously defined BLAME_COL_WIDTH=26 locally; it now
    // imports the shared constant from draw.rs which is 37. Assert the value
    // so any future accidental revert is caught immediately.
    assert_eq!(
        crate::ui::content::draw::BLAME_COL_WIDTH,
        37,
        "shared BLAME_COL_WIDTH must be 37 (not the old 26)"
    );
}

#[test]
fn render_inline_fallback_with_blame_does_not_panic() {
    let (mut app, dir) = render_app();
    let path = dir.path().join("f.txt");
    app.plugin_blame.insert(
        path,
        vec![
            format!("{:<37}", "Alice     fix memory leak"),
            format!("{:<37}", "Bob       add feature"),
            format!("{:<37}", "Carol     refactor"),
        ],
    );
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.show_blame = true;
        app.content = vec!["line1".to_string(), "line2".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "line1".to_string())],
            vec![(Style::default(), "line2".to_string())],
        ];
    });
    assert!(!screen.is_empty());
}
