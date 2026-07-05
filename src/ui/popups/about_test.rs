use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::Config;
use crate::ui::popups::draw_about;

fn make_app(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn buffer_text(app: &App) -> String {
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_about(f, app, f.area())).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn about_shows_title_and_close_hint() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);
    assert!(text.contains("About mantis"), "must show title: {text}");
    assert!(text.contains("Esc"), "must hint at closing via Esc: {text}");
}

#[test]
fn about_shows_version_from_cargo_pkg_when_no_release_info() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);
    // RELEASE is only populated at release-build time; in tests it falls back
    // to the crate's own Cargo.toml version.
    assert!(
        text.contains(env!("CARGO_PKG_VERSION")),
        "must show fallback crate version: {text}"
    );
}

#[test]
fn about_shows_license() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);
    assert!(
        text.contains("GPL-3.0-or-later"),
        "must show license: {text}"
    );
}

#[test]
fn about_hides_release_notes_when_no_release_info() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);
    // Without embedded release metadata there is no "what's new" body and no
    // release-url hint to show.
    if crate::release_info::RELEASE.is_none() {
        assert!(
            !text.contains("What's new"),
            "must not show 'What's new' without release info: {text}"
        );
        assert!(
            !text.contains("open release in browser"),
            "must not hint at opening a release url without one: {text}"
        );
    }
}
