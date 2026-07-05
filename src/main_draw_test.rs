use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::config::Config;
use crate::ui;

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mantis_main_draw_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn app_draw_tree_focus_does_not_panic() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_content_focus_does_not_panic() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\nworld\nline3\n").unwrap();
    let mut app = app_for(&dir);
    app.open_file(&file_path);
    app.focus = crate::app::Focus::Content;
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_side_by_side_diff_does_not_panic() {
    use std::process::Command;
    let dir = temp_dir();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    fs::write(dir.join("f.txt"), "alpha\nbeta\ngamma\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    fs::write(dir.join("f.txt"), "alpha\nBETA changed\ngamma\n").unwrap();
    git(&["commit", "-q", "-am", "change beta"]);

    // toggle_diff_side_by_side ships unbound by default (palette-only, see
    // Keymap::default); bind a key here to drive it directly.
    let mut config = Config::default();
    config.keys.toggle_diff_side_by_side = crate::config::bind(&["D"]);
    let mut app = App::new(dir.clone(), config, None, None).unwrap();
    app.open_file(&dir.join("f.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.is_diff, "history Enter should load a diff");
    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::empty()));
    assert!(app.diff_side_by_side);

    let backend = TestBackend::new(100, 20);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();

    let buffer = terminal.backend().buffer().clone();
    let has_divider = buffer.content().iter().any(|c| c.symbol() == "│");
    assert!(has_divider, "side-by-side diff should render a divider");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_search_open() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.focus = crate::app::Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert!(app.search.is_some());
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_theme_picker() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    assert!(app.theme_picker.is_some());
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_command_palette() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_help_open() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.show_help = true;
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_history_and_git_info() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".into()),
        ahead: 2,
        behind: 1,
        total_changed: 3,
        staged: 1,
        untracked: 2,
    });
    app.show_scroll_percentage = true;
    app.current_file = Some(dir.join("a.txt"));
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_in_file_search() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\nworld\n").unwrap();
    let mut app = app_for(&dir);
    app.open_file(&file_path);
    app.focus = crate::app::Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
    assert!(app.in_file_search.is_some());
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn app_draw_with_walk_errors_and_config_error() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    app.walk_errors = 3;
    app.config_error = Some("bad field: unknown_key".into());
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| ui::draw(f, &mut app)).unwrap();
    fs::remove_dir_all(&dir).ok();
}
