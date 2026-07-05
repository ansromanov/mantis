//! Binary entry point for `mantis`.
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
    event::{EnableMouseCapture, Event},
    execute,
    terminal::{enable_raw_mode, EnterAlternateScreen},
};
use ratatui::{backend::Backend, backend::CrosstermBackend, Terminal};

use crate::app::App;

mod actions;
mod ansi;
mod app;
mod command_palette;
mod command_usage;
mod config;
mod diff;
#[cfg(unix)]
mod event_source;
mod file;
mod fold;
mod git;
mod highlight;
mod list_picker;
mod pager;
mod plugin;
mod release_info;
mod search;
mod selection;
mod session;
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

/// Returns the value of a `--language <lang>`/`--language=<lang>` flag among
/// the process arguments, if present. Used to force syntax highlighting for
/// piped stdin content (pager mode) when first-line sniffing isn't enough.
fn parse_language_flag() -> Option<String> {
    parse_language_flag_from(std::env::args())
}

/// Parses the `--language` flag from an iterator of strings. Extracted for
/// testability, mirroring `parse_args_from`.
fn parse_language_flag_from<I>(args: I) -> Option<String>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    while let Some(a) = iter.next() {
        if let Some(v) = a.strip_prefix("--language=") {
            return Some(v.to_string());
        }
        if a == "--language" {
            return iter.next();
        }
    }
    None
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
            MetaAction::Help => "Usage: mantis [<path>]\n  \
                 <path>  File or directory to open (default: current dir)\n\n\
                 Options:\n  \
                 -h, --help, /?      Print this help\n  \
                 -V, --version       Print version\n  \
                 --language <lang>   Force the syntax used to highlight piped stdin\n\n\
                 Pager mode: with no <path> and stdin not a terminal, mantis reads\n\
                 stdin instead of a directory - diff-shaped input renders as a\n\
                 navigable side-by-side diff, anything else is syntax-highlighted:\n  \
                 git diff | mantis\n  \
                 kubectl logs pod | mantis\n"
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
/// (help/version) and exit, launch the TUI rooted at `root`, or read piped
/// stdin into a pager view.
enum Startup {
    /// Print this text to stdout and exit successfully.
    Print(String),
    /// Launch the UI for `root`, optionally revealing `file`.
    Launch {
        root: PathBuf,
        file: Option<PathBuf>,
    },
    /// Read stdin into the content pane instead of walking a directory
    /// (`git diff | mantis`). `root` still anchors the (collapsed) tree pane.
    Pager {
        root: PathBuf,
        language: Option<String>,
    },
}

/// Decides what to do with the parsed CLI argument. Pure and fully testable:
/// the only side-effecting work (terminal setup, reading stdin, the event
/// loop) is deferred to `main` based on the returned `Startup`.
fn plan_startup(
    arg: Option<PathBuf>,
    language: Option<String>,
    stdin_piped: bool,
) -> anyhow::Result<Startup> {
    if let Some(action) = meta_action(arg.as_deref()) {
        return Ok(Startup::Print(action.message()));
    }
    // Pager mode triggers when no real path argument was given (missing, or
    // flag-like — the same rule `resolve_input_path` uses to fall back to the
    // current dir) and stdin is a pipe rather than a terminal.
    let has_path_arg = arg
        .as_deref()
        .is_some_and(|a| !a.to_string_lossy().starts_with('-'));
    if stdin_piped && !has_path_arg {
        let root = resolve_input_path(None)?;
        return Ok(Startup::Pager { root, language });
    }
    let (root, file) = resolve_root_and_file(&resolve_input_path(arg)?);
    Ok(Startup::Launch { root, file })
}

/// A source of input events for the event loop. Abstracted so the loop can be
/// driven by a real terminal in production and a scripted queue in tests.
trait EventSource {
    /// Waits briefly (≈16 ms) for the next event. Returns `None` on timeout.
    fn next_event(&mut self) -> anyhow::Result<Option<Event>>;

    /// Returns an already-buffered event without waiting. `None` when none is
    /// immediately available. Never blocks.
    fn try_next_event(&mut self) -> anyhow::Result<Option<Event>>;
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

    fn try_next_event(&mut self) -> anyhow::Result<Option<Event>> {
        if crossterm::event::poll(Duration::ZERO)? {
            Ok(Some(crossterm::event::read()?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(unix)]
impl EventSource for event_source::RawEventSource {
    fn next_event(&mut self) -> anyhow::Result<Option<Event>> {
        self.next_raw_event().map_err(Into::into)
    }

    fn try_next_event(&mut self) -> anyhow::Result<Option<Event>> {
        self.try_next_raw_event().map_err(Into::into)
    }
}

fn main() -> anyhow::Result<()> {
    let stdin_piped = pager::is_piped_stdin();
    match plan_startup(parse_args(), parse_language_flag(), stdin_piped)? {
        Startup::Print(message) => {
            print!("{message}");
            Ok(())
        }
        Startup::Launch { root, file } => {
            let initial = file.map_or(InitialContent::None, InitialContent::File);
            launch_tui(root, initial)
        }
        Startup::Pager { root, language } => {
            // Read stdin to EOF before touching the terminal, mirroring how
            // `less`/`git`'s built-in pager behave.
            let bytes = pager::read_stdin_bytes()?;
            let parsed = pager::parse_pager_bytes(&bytes);
            launch_tui(root, InitialContent::Pager { parsed, language })
        }
    }
}

/// What the content pane should show right after startup: nothing (default),
/// a revealed path argument, or parsed piped-stdin content (pager mode).
enum InitialContent {
    None,
    File(PathBuf),
    Pager {
        parsed: pager::PagerContent,
        language: Option<String>,
    },
}

/// Puts the terminal into raw/alternate-screen mode, optionally enables the
/// kitty keyboard protocol, runs the app to completion, then restores the
/// terminal regardless of how the app exited. This is the only part of startup
/// that touches the real terminal, so it is not unit-tested.
/// Restores the terminal on drop so that any early `?` return during setup
/// (after raw mode is already enabled) still leaves the terminal usable.
/// Redundant with the explicit restore on the success path, but harmless
/// since `restore_terminal` is idempotent.
struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        crate::app::restore_terminal();
    }
}

fn launch_tui(root: PathBuf, initial: InitialContent) -> anyhow::Result<()> {
    // Pager mode reads fd 0 for content, so keyboard events must come from
    // the controlling terminal instead (the standard pager trick).
    #[cfg(unix)]
    let pager_mode = matches!(initial, InitialContent::Pager { .. });

    enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    crate::app::set_alternate_scroll(false);

    // Enable kitty keyboard protocol on terminals that support it.
    #[cfg(unix)]
    let keyboard_enhanced = event_source::push_keyboard_enhancement_flags().unwrap_or(false);

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Install panic hook right after terminal setup so that any panic (including
    // abort in release builds) restores the terminal to a usable state.
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        crate::app::restore_terminal();
        prev(info);
    }));

    // Use a trait-object event source so we can swap between the kitty-aware
    // raw parser (on Unix) and the regular crossterm source. crossterm's own
    // event source already reopens `/dev/tty` when stdin isn't a terminal, so
    // only the custom raw parser needs the explicit pager-mode fd.
    #[cfg(unix)]
    let mut events: Box<dyn EventSource> = if keyboard_enhanced {
        if pager_mode {
            Box::new(event_source::RawEventSource::for_tty())
        } else {
            Box::new(event_source::RawEventSource::new())
        }
    } else {
        Box::new(CrosstermEvents)
    };
    #[cfg(not(unix))]
    let mut events: Box<dyn EventSource> = Box::new(CrosstermEvents);

    let result = run_app(&mut terminal, root, initial, events.as_mut());

    result
}

/// Builds the app for `root`, applies `initial` (a revealed file, piped
/// stdin content, or nothing), runs the event loop against `terminal`, and
/// reports any config error after the loop exits. Generic over the backend
/// so tests can drive it with `TestBackend`.
fn run_app(
    terminal: &mut Terminal<impl Backend>,
    root: PathBuf,
    initial: InitialContent,
    events: &mut dyn EventSource,
) -> anyhow::Result<()> {
    let (cfg, cfg_path, cfg_error) = config::load(&root);
    let mut app = App::new(root, cfg, cfg_path, cfg_error)?;
    match initial {
        InitialContent::File(file) => app.open_and_reveal(&file),
        InitialContent::Pager { parsed, language } => app.open_pager_content(parsed, language),
        InitialContent::None => {}
    }
    // Drive tree/git refreshes from filesystem events rather than a blind timer.
    app.watch_root();

    let loop_result = run_event_loop(terminal, &mut app, events);

    // Persist session state (expanded dirs, open file, scroll pos, git mode).
    app.save_session();

    // Notify plugins of quit on every exit path, then shut them down.
    app.plugin_manager.on_quit();
    app.plugin_manager.deactivate_all();

    loop_result?;

    if let Some(err) = &app.config_error {
        eprintln!("mantis: ignoring invalid config: {err}");
    }

    Ok(())
}

/// Drives the event loop: renders the UI, pulls the next event from `events`,
/// dispatches it to the app, and calls `tick()` every frame.
///
/// When multiple events are buffered (e.g. a mouse-wheel burst), they are all
/// drained before the next render so the burst collapses into a single frame.
fn run_event_loop(
    terminal: &mut Terminal<impl Backend>,
    app: &mut App,
    events: &mut dyn EventSource,
) -> anyhow::Result<()> {
    loop {
        render_frame(terminal, app)?;

        if let Some(event) = events.next_event()? {
            dispatch_event(app, event);
            // Drain a burst (e.g. mouse-wheel) so it applies in one frame.
            let mut drained = 0;
            while let Some(event) = events.try_next_event()? {
                dispatch_event(app, event);
                drained += 1;
                if drained >= 256 {
                    break; // safety cap: never starve the render
                }
            }
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

#[cfg(test)]
#[path = "main_draw_test.rs"]
mod draw_tests;
