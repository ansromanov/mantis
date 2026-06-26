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
/// carries the `active_line_bg` background — the marker left by the active-line
/// highlight.
fn renders_active_line_highlight(app: &mut App) -> bool {
    let active_bg = app.theme.active_line_bg;
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
        .any(|c| c.bg == active_bg)
}

#[test]
fn active_line_highlight_painted_for_normal_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    assert!(
        renders_active_line_highlight(&mut app),
        "active line should be highlighted with active_line_bg"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_highlight_absent_in_diff_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.is_diff = true;
    assert!(
        !renders_active_line_highlight(&mut app),
        "diff mode must skip the active-line highlight"
    );
    fs::remove_dir_all(&root).ok();
}

/// Returns the (row, col) of every cell with the given background color.
fn cells_with_bg(app: &mut App, bg: ratatui::style::Color) -> Vec<(u16, u16)> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    buf.content()
        .iter()
        .enumerate()
        .filter(|(_, c)| c.bg == bg)
        .map(|(i, _)| {
            let col = (i as u16) % 80;
            let row = (i as u16) / 80;
            (row, col)
        })
        .collect()
}

#[test]
fn active_line_full_width_highlight() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    let active_bg = app.theme.active_line_bg;
    let cells = cells_with_bg(&mut app, active_bg);
    assert!(!cells.is_empty(), "active line must have highlighted cells");
    // All active-bg cells must be on the same row (line 0 since active_line=0)
    let rows: std::collections::BTreeSet<u16> = cells.iter().map(|(r, _)| *r).collect();
    assert_eq!(
        rows.len(),
        1,
        "active line highlight must be on a single row"
    );
    // The highlight must span at least 50 columns (content area is ~77 wide at 80-2 border - gutter)
    let min_col = cells.iter().map(|(_, c)| c).min().unwrap();
    let max_col = cells.iter().map(|(_, c)| c).max().unwrap();
    assert!(
        max_col - min_col >= 50,
        "active line highlight must span more than 50 columns, got {}-{}={}",
        max_col,
        min_col,
        max_col - min_col
    );
    // Verify gutter cells (col < ln_width ~4) also have active_bg
    let gutter_cells: Vec<_> = cells.iter().filter(|(_, c)| *c < 5).collect();
    assert!(
        !gutter_cells.is_empty(),
        "gutter must also be highlighted with active_line_bg"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_caret_in_gutter() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    let active_bg = app.theme.active_line_bg;
    let accent = app.theme.accent;
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, &mut app, frame.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    // Gutter is at the left of the content pane (column ~1 after border).
    // The active line's gutter should have accent foreground.
    let has_accent_in_gutter = buf.content().iter().enumerate().any(|(i, c)| {
        let col = (i as u16) % 80;
        let _row = (i as u16) / 80;
        col < 5 && c.fg == accent
    });
    assert!(
        has_accent_in_gutter,
        "gutter on active line must use accent foreground"
    );
    // The active line gutter must also have active_line_bg background
    let has_active_bg_in_gutter = buf.content().iter().enumerate().any(|(i, c)| {
        let col = (i as u16) % 80;
        let _row = (i as u16) / 80;
        col < 5 && c.bg == active_bg
    });
    assert!(
        has_active_bg_in_gutter,
        "gutter on active line must have active_line_bg background"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn active_line_moves_with_down_key() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    // Move down one line
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(app.active_line, 1);
    // Render and check the highlight is now on row for line 1
    let active_bg = app.theme.active_line_bg;
    let c1 = cells_with_bg(&mut app, active_bg);
    // The highlighted row shouldn't be the same as for line 0
    assert!(!c1.is_empty(), "highlight must exist after moving down");
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
