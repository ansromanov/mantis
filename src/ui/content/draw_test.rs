// Tests for active-line highlight logic in draw_content.
//
// Full rendering tests for draw_content are in content_test.rs.
// These tests cover the active-line highlight guard introduced by this PR.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;

use crate::app::{App, Focus};
use crate::config::Config;
use crate::ui::content::draw_content;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_draw_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let long: String = (1..=20).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn active_line_initialises_to_zero_on_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_saturates_at_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;

    for _ in 0..200 {
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    }
    let max = app.display_line_count().saturating_sub(1);
    assert_eq!(app.active_line, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_guard_skipped_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert!(app.is_diff);
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

/// Renders `draw_content` into an 80x24 backend and reports whether any cell
/// carries the `selection_bg` background — the marker left by the active-line
/// tint.
fn renders_active_line_tint(app: &mut App) -> bool {
    let sel_bg = app.theme.selection_bg;
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
        .any(|c| c.bg == sel_bg)
}

#[test]
fn active_line_tint_painted_for_normal_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert!(
        renders_active_line_tint(&mut app),
        "active line should be tinted with selection_bg"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_tint_absent_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.is_diff = true;
    assert!(
        !renders_active_line_tint(&mut app),
        "diff mode must skip the active-line tint"
    );
    fs::remove_dir_all(&root).ok();
}

/// Renders `draw_content` and returns the flattened text of the buffer.
fn render_to_string(app: &mut App) -> String {
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
fn plugin_content_is_rendered_for_current_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("long.txt");
    app.open_file(&path);
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(
            ratatui::style::Style::default(),
            "PLUGIN_RENDERED_MARKER".to_string(),
        )]],
    );
    app.plugin_content_text
        .insert(path, vec!["PLUGIN_RENDERED_MARKER".to_string()]);
    let out = render_to_string(&mut app);
    assert!(
        out.contains("PLUGIN_RENDERED_MARKER"),
        "plugin content must take precedence in the content pane"
    );
    fs::remove_dir_all(&root).ok();
}
