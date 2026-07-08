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
    let dir = std::env::temp_dir().join(format!("mantis_main_{}_{n}", std::process::id()));
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

    fn try_next_event(&mut self) -> anyhow::Result<Option<Event>> {
        // Pop the next event without blocking; None when the queue is empty.
        Ok(self.queue.pop_front())
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

    fn try_next_event(&mut self) -> anyhow::Result<Option<Event>> {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// clap CLI parsing tests
// ---------------------------------------------------------------------------

#[test]
fn cli_defaults() {
    let cli = Cli::try_parse_from(["mantis"]).unwrap();
    assert!(cli.path.is_none());
    assert!(cli.language.is_none());
    assert!(cli.completions.is_none());
    assert!(!cli.print_man_page);
    assert!(!cli.update);
}

#[test]
fn cli_parses_path() {
    let cli = Cli::try_parse_from(["mantis", "/some/path"]).unwrap();
    assert_eq!(cli.path, Some(PathBuf::from("/some/path")));
}

#[test]
fn cli_parses_language() {
    let cli = Cli::try_parse_from(["mantis", "--language", "rust"]).unwrap();
    assert_eq!(cli.language.as_deref(), Some("rust"));
}

#[test]
fn cli_parses_language_equals() {
    let cli = Cli::try_parse_from(["mantis", "--language=python"]).unwrap();
    assert_eq!(cli.language.as_deref(), Some("python"));
}

#[test]
fn cli_parses_language_short() {
    let cli = Cli::try_parse_from(["mantis", "-l", "go"]).unwrap();
    assert_eq!(cli.language.as_deref(), Some("go"));
}

#[test]
fn cli_parses_completions() {
    let cli = Cli::try_parse_from(["mantis", "--completions", "bash"]).unwrap();
    assert_eq!(cli.completions.as_deref(), Some("bash"));
}

#[test]
fn cli_parses_print_man_page() {
    let cli = Cli::try_parse_from(["mantis", "--print-man-page"]).unwrap();
    assert!(cli.print_man_page);
}

#[test]
fn cli_parses_update() {
    let cli = Cli::try_parse_from(["mantis", "--update"]).unwrap();
    assert!(cli.update);
}

#[test]
fn cli_parses_path_and_language() {
    let cli = Cli::try_parse_from(["mantis", "--language", "rust", "/some/path"]).unwrap();
    assert_eq!(cli.language.as_deref(), Some("rust"));
    assert_eq!(cli.path, Some(PathBuf::from("/some/path")));
}

#[test]
fn cli_rejects_unknown_flag() {
    let result = Cli::try_parse_from(["mantis", "--bogus"]);
    assert!(result.is_err());
}

#[test]
fn cli_allows_any_completions_value() {
    // clap stores the value as a string — validation happens in print_completions.
    let cli = Cli::try_parse_from(["mantis", "--completions", "csh"]).unwrap();
    assert_eq!(cli.completions.as_deref(), Some("csh"));
}

// ---------------------------------------------------------------------------
// print_completions / print_man_page
// ---------------------------------------------------------------------------

#[test]
fn print_completions_bash_produces_output() {
    let mut buf = Vec::new();
    let shell = clap_complete::Shell::Bash;
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("mantis"));
    assert!(output.contains("--language") || output.contains("--completions"));
}

#[test]
fn print_completions_zsh_produces_output() {
    let mut buf = Vec::new();
    let shell = clap_complete::Shell::Zsh;
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty());
}

#[test]
fn print_completions_errors_on_unsupported_shell() {
    let err = print_completions("csh").unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("csh"));
}

#[test]
fn print_man_page_produces_output() {
    // Render the man page to a buffer and verify it has expected sections.
    let cmd = Cli::command();
    let mut buf = Vec::new();
    let man = clap_mangen::Man::new(cmd);
    man.render(&mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("mantis"));
    assert!(output.contains(".TH") || output.contains(".SH"));
}

// ---------------------------------------------------------------------------
// resolve_root_and_file
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// resolve_input_path
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// plan_startup
// ---------------------------------------------------------------------------

#[test]
fn plan_startup_directory_returns_launch() {
    let dir = temp_dir();
    let startup = plan_startup(Some(dir.clone()), None, false).unwrap();
    match startup {
        Startup::Launch { root, file, .. } => {
            assert_eq!(root, dir);
            assert!(file.is_none());
        }
        _ => panic!("expected Launch for a directory"),
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_startup_file_returns_launch_with_file() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\n").unwrap();
    let canonical = file_path.canonicalize().unwrap();
    let startup = plan_startup(Some(canonical.clone()), None, false).unwrap();
    match startup {
        Startup::Launch { root, file, .. } => {
            assert_eq!(root, dir);
            assert_eq!(file, Some(canonical));
        }
        _ => panic!("expected Launch for a file"),
    }
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_startup_missing_path_errors() {
    let missing = std::env::temp_dir().join("tv_plan_missing_xyz_98765");
    assert!(plan_startup(Some(missing), None, false).is_err());
}

#[test]
fn plan_startup_piped_stdin_with_no_path_returns_pager() {
    let startup = plan_startup(None, Some("rust".to_string()), true).unwrap();
    match startup {
        Startup::Pager { root, language } => {
            assert_eq!(root, PathBuf::from(".").canonicalize().unwrap());
            assert_eq!(language.as_deref(), Some("rust"));
        }
        _ => panic!("expected Pager when stdin is piped and no path is given"),
    }
}

#[test]
fn plan_startup_piped_stdin_with_flag_arg_returns_pager() {
    // A flag-like first arg (e.g. `mantis --language rust < file`) must not be
    // mistaken for a path argument.
    let startup = plan_startup(Some(PathBuf::from("--language")), None, true).unwrap();
    assert!(matches!(startup, Startup::Pager { .. }));
}

#[test]
fn plan_startup_piped_stdin_with_real_path_returns_launch() {
    // An explicit path argument takes precedence over piped stdin.
    let dir = temp_dir();
    let startup = plan_startup(Some(dir.clone()), None, true).unwrap();
    assert!(matches!(startup, Startup::Launch { .. }));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn plan_startup_no_path_without_piped_stdin_returns_launch() {
    let startup = plan_startup(None, None, false).unwrap();
    match startup {
        Startup::Launch { root, file, .. } => {
            assert_eq!(root, PathBuf::from(".").canonicalize().unwrap());
            assert!(file.is_none());
        }
        _ => panic!("expected Launch when stdin is a tty"),
    }
}

// ---------------------------------------------------------------------------
// dispatch_event
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// event loop
// ---------------------------------------------------------------------------

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
    // ctrl+f opens the full-text content search overlay.
    app.focus = crate::app::Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL));
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
    // command_palette = ctrl+p; matching is case-insensitive so an
    // uppercase event (CapsLock / stray Shift) must open it too.
    app.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.command_palette.is_none());
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

// ---------------------------------------------------------------------------
// run_app
// ---------------------------------------------------------------------------

#[test]
fn run_app_builds_and_runs_to_quit() {
    let dir = temp_dir();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    run_app(
        &mut terminal,
        dir.clone(),
        InitialContent::None,
        &mut events,
    )
    .unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_app_opens_and_reveals_file() {
    let dir = temp_dir();
    let file_path = dir.join("a.txt");
    fs::write(&file_path, "hello\nworld\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    // Opening a file switches focus to Content, where the tree-scoped `q`
    // quit binding doesn't apply; use the global ctrl+c binding instead.
    let mut events = ScriptedEvents::new(vec![Event::Key(KeyEvent::new(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL,
    ))]);
    run_app(
        &mut terminal,
        dir.clone(),
        InitialContent::File(file_path),
        &mut events,
    )
    .unwrap();
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn run_app_surfaces_config_error_without_failing() {
    let dir = temp_dir();
    fs::write(dir.join("mantis.toml"), "garbage [[[ = 1").unwrap();
    fs::write(dir.join("a.txt"), "hello\n").unwrap();
    let backend = TestBackend::new(80, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut events = ScriptedEvents::new(vec![key_event('q')]);
    // A bad config is reported (to stderr) but must not abort the run.
    run_app(
        &mut terminal,
        dir.clone(),
        InitialContent::None,
        &mut events,
    )
    .unwrap();
    fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------------
// misc helpers
// ---------------------------------------------------------------------------

#[test]
fn config_error_surfaces_from_invalid_toml() {
    let dir = temp_dir();
    fs::write(dir.join("mantis.toml"), "garbage [[[ = 1").unwrap();
    let (_cfg, _path, err) = crate::config::load(&dir);
    assert!(err.is_some());
    assert!(err.unwrap().contains("mantis.toml"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn restore_terminal_is_idempotent() {
    // Calling restore_terminal() multiple times must not panic even when the
    // terminal is not in raw/alternate-screen mode.
    crate::app::restore_terminal();
    crate::app::restore_terminal();
}

#[test]
fn meta_action_recognizes_telemetry_status_flag() {
    let arg = PathBuf::from("--telemetry-status");
    assert!(matches!(
        meta_action(Some(&arg)),
        Some(MetaAction::TelemetryStatus)
    ));
}

#[test]
fn plan_startup_telemetry_status_returns_print() {
    let startup = plan_startup(Some(PathBuf::from("--telemetry-status")), None, false).unwrap();
    match startup {
        Startup::Print(msg) => {
            assert!(msg.contains("Telemetry:"));
            assert!(msg.contains("Directory:"));
        }
        _ => panic!("expected Print for --telemetry-status"),
    }
}
