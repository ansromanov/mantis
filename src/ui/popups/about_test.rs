use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::config::{Config, GitConfig};
use crate::ui::popups::about::fit_changelog;
use crate::ui::popups::draw_about;

fn make_app(root: &std::path::Path) -> App {
    let cfg = Config {
        git: GitConfig {
            status: false,
            ..Default::default()
        },
        ..Config::default()
    };
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
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
fn about_shows_version_matching_release_selection_logic() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);
    // Mirrors draw_about's own fallback: release-info version when embedded
    // metadata is present, otherwise the crate's Cargo.toml version. Asserting
    // against this same selection (rather than assuming RELEASE is absent in
    // tests) keeps the test correct whether or not release-info.toml is
    // populated in the checkout.
    let expected_version = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.version.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    assert!(
        text.contains(expected_version),
        "must show version {expected_version:?}: {text}"
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
fn about_release_notes_visibility_matches_release_info() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let text = buffer_text(&app);

    let whats_new = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.whats_new.as_str())
        .unwrap_or("");
    let has_url = crate::release_info::RELEASE
        .as_ref()
        .map(|r| !r.release_url.is_empty())
        .unwrap_or(false);

    assert_eq!(
        text.contains("What's new"),
        !whats_new.is_empty(),
        "'What's new' section visibility must match whether release-info has a changelog blurb: {text}"
    );
    assert_eq!(
        text.contains("open release in browser"),
        has_url,
        "release-url hint visibility must match whether release-info has a release_url: {text}"
    );
}

#[test]
fn fit_changelog_keeps_all_entries_within_budget() {
    let entries = ["a", "b", "c"];
    let (shown, remaining) = fit_changelog(&entries, 5);
    assert_eq!(shown, &entries[..]);
    assert_eq!(remaining, 0);
}

#[test]
fn fit_changelog_truncates_and_reserves_a_row_for_the_marker() {
    let entries = ["a", "b", "c", "d", "e"];
    // Budget of 3 rows: 2 shown + 1 reserved for the "N more" marker.
    let (shown, remaining) = fit_changelog(&entries, 3);
    assert_eq!(shown, &entries[..2]);
    assert_eq!(remaining, 3);
}

#[test]
fn fit_changelog_zero_budget_shows_nothing_but_counts_all_as_remaining() {
    let entries = ["a", "b"];
    let (shown, remaining) = fit_changelog(&entries, 0);
    assert!(shown.is_empty());
    assert_eq!(remaining, 2);
}
