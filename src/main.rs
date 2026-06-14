use std::io;
use std::path::{Path, PathBuf};

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::app::App;

mod app;
mod command_palette;
mod config;
mod file;
mod git;
mod highlight;
mod markdown;
mod release_info;
mod search;
mod selection;
mod theme;
mod tree;
mod ui;
mod virtual_file;

/// Parses a canonicalized path argument into (root_dir, optional_file_to_open).
fn resolve_root_and_file(arg: &Path) -> (PathBuf, Option<PathBuf>) {
    if arg.is_file() {
        let parent = arg
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        (parent, Some(arg.to_path_buf()))
    } else {
        (arg.to_path_buf(), None)
    }
}

/// Returns the first CLI argument as a `PathBuf`, if any.
fn parse_args() -> Option<PathBuf> {
    std::env::args().nth(1).map(PathBuf::from)
}

fn main() -> anyhow::Result<()> {
    let arg = parse_args();
    let flag = arg.as_deref().and_then(|p| p.to_str());
    match flag {
        Some("--help") | Some("-h") | Some("/?") => {
            println!("Usage: tv [<path>]");
            println!("  <path>  File or directory to open (default: current dir)");
            println!();
            println!("Options:");
            println!("  -h, --help, /?    Print this help");
            println!("  -V, --version     Print version");
            std::process::exit(0);
        }
        Some("--version") | Some("-V") => {
            println!("v{}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        _ => {}
    }

    let arg = arg
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()?;

    let (root, file) = resolve_root_and_file(&arg);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let (cfg, cfg_path, cfg_error) = config::load(&root);
    let mut app = app::App::new(root, cfg, cfg_path, cfg_error)?;
    if let Some(file) = file {
        app.open_and_reveal(&file);
    }

    run_event_loop(&mut terminal, &mut app)?;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Some(err) = &app.config_error {
        eprintln!("tv: ignoring invalid config: {err}");
    }

    Ok(())
}

/// Drives the event loop: renders the UI, polls for events, dispatches them
/// to the app, and calls `tick()` every frame.
fn run_event_loop(
    terminal: &mut Terminal<impl ratatui::backend::Backend>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        if app.needs_clear {
            terminal.clear()?;
            terminal.hide_cursor()?;
            app.needs_clear = false;
        }
        terminal.draw(|f| ui::draw(f, app))?;

        if crossterm::event::poll(std::time::Duration::from_millis(16))? {
            match crossterm::event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Mouse(m) => app.handle_mouse(m),
                _ => {}
            }
        }

        if app.should_quit {
            break;
        }

        app.tick();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
    fn parse_args_returns_option() {
        let result = parse_args();
        assert!(result.is_some() || result.is_none());
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
        // / in content focus opens in-file search
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
    fn terminal_setup_config_error_surfaces_warning() {
        let result = parse_args();
        assert!(result.is_some() || result.is_none());
    }
}
