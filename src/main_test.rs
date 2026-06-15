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
    let dir = std::env::temp_dir().join(format!("tv_main_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn resolve_root_and_file_with_directory() {
    let dir = temp_dir();
    let (root, file) = resolve_root_and_file(&dir);
    assert_eq!(root, dir);
    assert!(file.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn resolve_root_and_file_with_file() {
    let dir = temp_dir();
    let file_path = dir.join("test.txt");
    fs::write(&file_path, "content").unwrap();
    let canonical = file_path.canonicalize().unwrap();
    let (root, file) = resolve_root_and_file(&canonical);
    assert_eq!(root, dir);
    assert_eq!(file, Some(canonical));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_args_returns_none_with_no_args() {
    let result = parse_args_from(std::iter::empty::<String>());
    assert!(result.is_none());
}

#[test]
fn parse_args_returns_first_arg() {
    let result = parse_args_from(["program", "some/path"].into_iter().map(String::from));
    assert_eq!(result, Some(PathBuf::from("some/path")));
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

    let mut app = app_for(&dir);
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
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
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
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
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
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
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

#[test]
fn event_loop_key_quit_sets_should_quit() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    assert!(!app.should_quit);
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
    assert!(app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn event_loop_key_search_toggles_search() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    assert!(app.search.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.search.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.search.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn event_loop_key_theme_toggles_picker() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    assert!(app.theme_picker.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    assert!(app.theme_picker.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.theme_picker.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn event_loop_key_help_toggles() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    assert!(!app.show_help);
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(app.show_help);
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(!app.show_help);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn event_loop_key_command_palette_toggles() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    assert!(app.command_palette.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.command_palette.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn config_error_surfaces_from_invalid_toml() {
    let dir = temp_dir();
    fs::write(dir.join("tv.toml"), "garbage [[[ = 1").unwrap();
    let (_cfg, _path, err) = crate::config::load(&dir);
    assert!(err.is_some());
    assert!(err.unwrap().contains("tv.toml"));
    fs::remove_dir_all(&dir).ok();
}
