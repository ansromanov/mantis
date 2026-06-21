//! Binary entry point for `tv`.
//!
//! Handles the command-line surface (`--help`/`--version` and an optional
//! file-or-directory path argument), then sets up the terminal: enables raw
//! mode, enters the alternate screen, and turns on mouse capture. It loads the
//! config, constructs the [`App`], installs the filesystem watcher, and runs the
//! synchronous render/poll/dispatch loop in `run_event_loop` until the app asks
//! to quit. On exit it restores the terminal to its original state and prints
//! any deferred config-load warning. This is the only module that uses `anyhow`
//! freely alongside `App::new`; everything below degrades errors into UI
//! messages instead of bubbling them up to here.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::Backend, backend::CrosstermBackend, Terminal};

use crate::app::App;

mod ansi;
mod app;
mod command_palette;
mod config;
mod diff;
mod file;
mod git;
mod highlight;
mod markdown;
mod plugin;
mod release_info;
mod search;
mod selection;
mod theme;
mod tree;
mod ui;
mod virtual_file;
mod yaml_fold;

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
    parse_args_from(std::env::args())
}

/// Parses the first argument from an iterator of strings. Extracted for
/// testability: tests can inject an arbitrary argument list.
fn parse_args_from<I>(args: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = String>,
{
    args.into_iter().nth(1).map(PathBuf::from)
}

/// A meta CLI action that prints information and exits before launching the UI.
enum MetaAction {
    Help,
    Version,
}

impl MetaAction {
    /// The text printed to stdout for this action.
    fn message(&self) -> String {
        match self {
            MetaAction::Help => "Usage: tv [<path>]\n  \
                 <path>  File or directory to open (default: current dir)\n\n\
                 Options:\n  \
                 -h, --help, /?    Print this help\n  \
                 -V, --version     Print version\n"
                .to_string(),
            MetaAction::Version => format!("v{}\n", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Classifies a CLI argument as a meta action (help/version), if it is one.
fn meta_action(arg: Option<&Path>) -> Option<MetaAction> {
    match arg.and_then(|p| p.to_str()) {
        Some("--help") | Some("-h") | Some("/?") => Some(MetaAction::Help),
        Some("--version") | Some("-V") => Some(MetaAction::Version),
        _ => None,
    }
}

/// Resolves the optional path argument to a canonical path: flag-like args are
/// ignored and a missing arg defaults to the current directory.
fn resolve_input_path(arg: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let path = arg
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(path.canonicalize()?)
}

/// What `main` should do once arguments are parsed: either print a meta message
/// (help/version) and exit, or launch the TUI rooted at `root`.
enum Startup {
    /// Print this text to stdout and exit successfully.
    Print(String),
    /// Launch the UI for `root`, optionally revealing `file`.
    Launch {
        root: PathBuf,
        file: Option<PathBuf>,
    },
}

/// Decides what to do with the parsed CLI argument. Pure and fully testable:
/// the only side-effecting work (terminal setup, the event loop) is deferred to
/// `main` based on the returned `Startup`.
fn plan_startup(arg: Option<PathBuf>) -> anyhow::Result<Startup> {
    if let Some(action) = meta_action(arg.as_deref()) {
        return Ok(Startup::Print(action.message()));
    }
    let (root, file) = resolve_root_and_file(&resolve_input_path(arg)?);
    Ok(Startup::Launch { root, file })
}

/// A source of input events for the event loop. Abstracted so the loop can be
/// driven by a real terminal in production and a scripted queue in tests.
trait EventSource {
    /// Returns the next available event, or `None` if none arrived in time.
    fn next_event(&mut self) -> anyhow::Result<Option<Event>>;
}

/// Production event source backed by crossterm's terminal event queue.
struct CrosstermEvents;

impl EventSource for CrosstermEvents {
    fn next_event(&mut self) -> anyhow::Result<Option<Event>> {
        if crossterm::event::poll(Duration::from_millis(16))? {
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    }
}

fn main() -> anyhow::Result<()> {
    match plan_startup(parse_args())? {
        Startup::Print(message) => {
            print!("{message}");
            Ok(())
        }
        Startup::Launch { root, file } => launch_tui(root, file),
    }
}

/// Puts the terminal into raw/alternate-screen mode, runs the app to completion,
/// then restores the terminal regardless of how the app exited. This is the only
/// part of startup that touches the real terminal, so it is not unit-tested.
fn launch_tui(root: PathBuf, file: Option<PathBuf>) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let mut events = CrosstermEvents;
    let result = run_app(&mut terminal, root, file, &mut events);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Builds the app for `root` (optionally revealing `file`), runs the event loop
/// against `terminal`, and reports any config error after the loop exits.
/// Generic over the backend so tests can drive it with `TestBackend`.
fn run_app(
    terminal: &mut Terminal<impl Backend>,
    root: PathBuf,
    file: Option<PathBuf>,
    events: &mut impl EventSource,
) -> anyhow::Result<()> {
    let (cfg, cfg_path, cfg_error) = config::load(&root);
    let mut app = App::new(root, cfg, cfg_path, cfg_error)?;
    if let Some(file) = file {
        app.open_and_reveal(&file);
    }
    // Drive tree/git refreshes from filesystem events rather than a blind timer.
    app.watch_root();

    let loop_result = run_event_loop(terminal, &mut app, events);

    // Notify plugins of quit on every exit path, then shut them down.
    app.plugin_manager.on_quit();
    app.plugin_manager.deactivate_all();

    loop_result?;

    if let Some(err) = &app.config_error {
        eprintln!("tv: ignoring invalid config: {err}");
    }

    Ok(())
}

/// Drives the event loop: renders the UI, pulls the next event from `events`,
/// dispatches it to the app, and calls `tick()` every frame.
fn run_event_loop(
    terminal: &mut Terminal<impl Backend>,
    app: &mut App,
    events: &mut impl EventSource,
) -> anyhow::Result<()> {
    loop {
        render_frame(terminal, app)?;

        if let Some(event) = events.next_event()? {
            dispatch_event(app, event);
        }

        if app.should_quit {
            break;
        }

        app.tick();
    }
    Ok(())
}

/// Clears the terminal when requested, then renders one frame of the UI.
fn render_frame(terminal: &mut Terminal<impl Backend>, app: &mut App) -> anyhow::Result<()> {
    if app.needs_clear {
        terminal.clear()?;
        terminal.hide_cursor()?;
        app.needs_clear = false;
    }
    terminal.draw(|f| ui::draw(f, app))?;
    Ok(())
}

/// Dispatches a single terminal event to the app's key/mouse handlers.
fn dispatch_event(app: &mut App, event: Event) {
    match event {
        Event::Key(key) => app.handle_key(key),
        Event::Mouse(m) => app.handle_mouse(m),
        _ => {}
    }
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
