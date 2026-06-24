use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;

use crate::app::App;
use crate::config::Config;

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

/// Scripted event source that yields a fixed queue of events, then `None`.
/// Lets tests drive `run_event_loop`/`run_app` without a real terminal.
struct ScriptedEvents {
    queue: std::collections::VecDeque<Event>,
}

impl ScriptedEvents {
    fn new(events: Vec<Event>) -> Self {
        Self {
            queue: events.into(),
        }
    }
}

impl EventSource for ScriptedEvents {
    fn next_event(&mut self) -> anyhow::Result<Option<Event>> {
        // Fail fast instead of spinning forever if a test forgets to quit.
        match self.queue.pop_front() {
            Some(event) => Ok(Some(event)),
            None => anyhow::bail!("scripted events exhausted without quitting"),
        }
    }
}

fn key_event(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()))
}

fn esc_event() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()))
}

/// Event source that reports one idle frame (`None`) before yielding a quit key,
/// exercising the loop's no-event `tick()` branch.
struct IdleThenQuit {
    yielded_idle: bool,
    quit: bool,
}

impl IdleThenQuit {
    fn new() -> Self {
        Self {
            yielded_idle: false,
            quit: false,
        }
    }
}

impl EventSource for IdleThenQuit {
    fn next_event(&mut self) -> anyhow::Result<Option<Event>> {
        if !self.yielded_idle {
            self.yielded_idle = true;
            return Ok(None);
        }
        if !self.quit {
            self.quit = true;
            return Ok(Some(key_event('q')));
        }
        anyhow::bail!("idle source exhausted without quitting")
    }
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
    // Switch to Content focus + no file so '/' opens SearchState.
    app.focus = crate::app::Focus::Content;
    app.current_file = None;
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
fn meta_action_recognizes_help_flags() {
    for flag in ["--help", "-h", "/?"] {
        let arg = PathBuf::from(flag);
        assert!(matches!(meta_action(Some(&arg)), Some(MetaAction::Help)));
    }
}

#[test]
fn meta_action_recognizes_version_flags() {
    for flag in ["--version", "-V"] {
        let arg = PathBuf::from(flag);
        assert!(matches!(meta_action(Some(&arg)), Some(MetaAction::Version)));
    }
}

#[test]
fn meta_action_none_for_path_or_missing() {
    assert!(meta_action(None).is_none());
    let arg = PathBuf::from("some/path");
    assert!(meta_action(Some(&arg)).is_none());
}

#[test]
fn meta_action_messages_have_expected_content() {
    assert!(MetaAction::Help.message().contains("Usage: tv"));
    let version = MetaAction::Version.message();
    assert!(version.starts_with('v'));
    assert!(version.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn resolve_input_path_defaults_to_current_dir() {
    let resolved = resolve_input_path(None).unwrap();
    assert_eq!(resolved, PathBuf::from(".").canonicalize().unwrap());
}

#[test]
fn resolve_input_path_ignores_flag_like_args() {
    let resolved = resolve_input_path(Some(PathBuf::from("--bogus"))).unwrap();
    assert_eq!(resolved, PathBuf::from(".").canonicalize().unwrap());
}

#[test]
fn resolve_input_path_canonicalizes_existing_path() {
    let dir = temp_dir();
    let resolved = resolve_input_path(Some(dir.clone())).unwrap();
    assert_eq!(resolved, dir);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn resolve_input_path_errors_on_missing_path() {
    let missing = std::env::temp_dir().join("tv_definitely_missing_xyz_12345");
    assert!(resolve_input_path(Some(missing)).is_err());
}

#[test]
fn dispatch_event_routes_key_to_handler() {
    let dir = temp_dir();
    let mut app = app_for(&dir);
    dispatch_event(&mut app, key_event('q'));
    assert!(app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn dispatch_event_ignores_non_key_mouse_events() {
    let dir = temp_dir();
    let mut app = app_for(&dir);
    dispatch_event(&mut app, Event::FocusGained);
    assert!(!app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn render_frame_clears_when_requested() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    app.needs_clear = true;
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    render_frame(&mut terminal, &mut app).unwrap();
    assert!(!app.needs_clear);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_event_loop_quits_on_q() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    run_event_loop(&mut terminal, &mut app, &mut events).unwrap();
    assert!(app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_event_loop_processes_events_before_quit() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    // Open search, close it, then quit; the loop renders between each event.
    let mut events = ScriptedEvents::new(vec![key_event('/'), esc_event(), key_event('q')]);
    run_event_loop(&mut terminal, &mut app, &mut events).unwrap();
    assert!(app.should_quit);
    assert!(app.search.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_event_loop_handles_idle_none_event() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    // An idle frame (None) then quit exercises the no-event tick() path.
    let mut events = IdleThenQuit::new();
    run_event_loop(&mut terminal, &mut app, &mut events).unwrap();
    assert!(app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_app_builds_and_runs_to_quit() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    run_app(&mut terminal, dir.clone(), None, &mut events).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_app_opens_and_reveals_file() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\nworld\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    run_app(&mut terminal, dir.clone(), Some(file_path), &mut events).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_app_surfaces_config_error_without_failing() {
    let dir = temp_dir();
    fs::write(dir.join("tv.toml"), "garbage [[[ = 1").unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    // A bad config is reported (to stderr) but must not abort the run.
    run_app(&mut terminal, dir.clone(), None, &mut events).unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn parse_args_reads_process_args_without_panicking() {
    // Smoke test for the std::env::args wrapper; the test harness passes its own
    // binary name, so the first user arg may be present or absent.
    let _ = parse_args();
}

#[test]
fn dispatch_event_routes_mouse_event() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let mut app = app_for(&dir);
    let mouse = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    });
    // Just needs to route to handle_mouse without panicking.
    dispatch_event(&mut app, mouse);
    assert!(!app.should_quit);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn crossterm_events_polls_without_panicking() {
    // Exercises the production event source. Under the test harness stdin is not
    // a tty, so poll may time out (Ok(None)) or error; either is acceptable here
    // -- we only assert it never yields a spurious event and never panics.
    let mut events = CrosstermEvents;
    if let Ok(event) = events.next_event() {
        assert!(event.is_none());
    }
}

#[test]
fn plan_startup_help_returns_print() {
    let startup = plan_startup(Some(PathBuf::from("--help"))).unwrap();
    match startup {
        Startup::Print(msg) => assert!(msg.contains("Usage: tv")),
        Startup::Launch { .. } => panic!("expected Print for --help"),
    }
}

#[test]
fn plan_startup_version_returns_print() {
    let startup = plan_startup(Some(PathBuf::from("--version"))).unwrap();
    match startup {
        Startup::Print(msg) => assert!(msg.starts_with('v')),
        Startup::Launch { .. } => panic!("expected Print for --version"),
    }
}

#[test]
fn plan_startup_directory_returns_launch() {
    let dir = temp_dir();
    let startup = plan_startup(Some(dir.clone())).unwrap();
    match startup {
        Startup::Launch { root, file } => {
            assert_eq!(root, dir);
            assert!(file.is_none());
        }
        Startup::Print(_) => panic!("expected Launch for a directory"),
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_startup_file_returns_launch_with_file() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\n").unwrap();
    let canonical = file_path.canonicalize().unwrap();
    let startup = plan_startup(Some(canonical.clone())).unwrap();
    match startup {
        Startup::Launch { root, file } => {
            assert_eq!(root, dir);
            assert_eq!(file, Some(canonical));
        }
        Startup::Print(_) => panic!("expected Launch for a file"),
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_startup_missing_path_errors() {
    let missing = std::env::temp_dir().join("tv_plan_missing_xyz_98765");
    assert!(plan_startup(Some(missing)).is_err());
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
