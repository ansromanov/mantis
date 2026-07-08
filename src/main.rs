//! Binary entry point for `mantis`.
//!
//! Handles the command-line surface using clap (positional path argument,
//! `--language`, `--completions`, `--print-man-page`, `--update`,
//! `--help`/`--version`), then sets up the terminal: enables raw mode, enters the
//! alternate screen, and turns on mouse capture. It loads the config, constructs
//! the [`App`], installs the filesystem watcher, and runs the synchronous
//! render/poll/dispatch loop in `run_event_loop` until the app asks to quit. On
//! exit it restores the terminal to its original state and prints any deferred
//! config-load warning. This is the only module that uses `anyhow` freely
//! alongside `App::new`; everything below degrades errors into UI messages
//! instead of bubbling them up to here.

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::CommandFactory;
use clap::Parser;
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
mod diagnostics;
mod diff;
mod event_source;
mod file;
mod fold;
mod git;
mod highlight;
mod list_picker;
mod pager;
mod plugin;
mod release_info;
mod scroll;
mod search;
mod selection;
mod session;
mod telemetry;
mod theme;
mod tree;
mod ui;
mod update;
mod virtual_file;
mod yaml_fold;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// A fast terminal file tree viewer with syntax highlighting, fuzzy search,
/// and mouse support.
#[derive(Parser)]
#[command(
    name = "mantis",
    version,
    about,
    long_about = "A fast terminal file tree viewer — navigate filesystems, preview \
                  files with syntax highlighting, fuzzy-search files and content, \
                  browse git history, and switch themes.\n\
                  \n\
                  Pager mode: with no <PATH> and stdin not a terminal, mantis reads \
                  stdin instead of a directory — diff-shaped input renders as a \
                  navigable side-by-side diff, anything else is syntax-highlighted:\n  \
                  git diff | mantis\n  \
                  kubectl logs pod | mantis",
    max_term_width = 80
)]
struct Cli {
    /// File or directory to open (default: current directory)
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Force the syntax highlighting language for piped stdin
    #[arg(long = "language", short = 'l', value_name = "LANG")]
    language: Option<String>,

    /// Generate shell completions (bash, zsh, fish, powershell)
    #[arg(long = "completions", value_name = "SHELL")]
    completions: Option<String>,

    /// Print the man page to stdout
    #[arg(long = "print-man-page")]
    print_man_page: bool,

    /// Self-update to the latest release
    #[arg(long = "update")]
    update: bool,

    /// Print telemetry status and directory
    #[arg(long = "telemetry-status")]
    telemetry_status: bool,
}

/// Prints shell completions for the given shell to stdout.
fn print_completions(shell: &str) -> anyhow::Result<()> {
    let shell = match shell {
        "bash" => clap_complete::Shell::Bash,
        "zsh" => clap_complete::Shell::Zsh,
        "fish" => clap_complete::Shell::Fish,
        "powershell" => clap_complete::Shell::PowerShell,
        other => anyhow::bail!("unsupported shell: {other} (use bash, zsh, fish, or powershell)"),
    };
    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());
    Ok(())
}

/// Prints the man page to stdout.
fn print_man_page() -> anyhow::Result<()> {
    let cmd = Cli::command();
    let mut stdout = std::io::stdout();
    let man = clap_mangen::Man::new(cmd);
    man.render(&mut stdout)?;
    Ok(())
}

/// Prints the telemetry status to stdout.
fn print_telemetry_status() -> anyhow::Result<()> {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (cfg, _, _) = config::load(&current_dir);
    let status = if cfg.telemetry.enabled {
        "active"
    } else {
        "disabled"
    };
    let dir = crate::session::state_dir()
        .map(|d| d.join("telemetry"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("Telemetry: {status}\nDirectory: {dir}");
    Ok(())
}

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

/// Resolves the optional path argument to a canonical path: flag-like args are
/// ignored and a missing arg defaults to the current directory.
fn resolve_input_path(arg: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let path = arg
        .filter(|a| !a.to_string_lossy().starts_with('-'))
        .unwrap_or_else(|| PathBuf::from("."));
    Ok(path.canonicalize()?)
}

/// What `main` should do once arguments are parsed: launch the TUI for `root`,
/// optionally revealing `file`, or read piped stdin into a pager view.
enum Startup {
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
    path: Option<PathBuf>,
    language: Option<String>,
    stdin_piped: bool,
) -> anyhow::Result<Startup> {
    // Pager mode triggers when no real path argument was given (missing, or
    // flag-like — the same rule `resolve_input_path` uses to fall back to the
    // current dir) and stdin is a pipe rather than a terminal.
    let has_path_arg = path
        .as_deref()
        .is_some_and(|a| !a.to_string_lossy().starts_with('-'));
    if stdin_piped && !has_path_arg {
        let root = resolve_input_path(None)?;
        return Ok(Startup::Pager { root, language });
    }
    let (root, file) = resolve_root_and_file(&resolve_input_path(path)?);
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

/// On Windows, when stdin is piped (pager mode), redirect the process's
/// standard input handle to `CONIN$` (the console input device) so that
/// crossterm's event system continues to receive keyboard input after the
/// pipe has been drained. Mirrors the Unix `/dev/tty` reopening trick that
/// `less` and other pagers use.
#[cfg(windows)]
fn redirect_stdin_to_console() -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;

    // Open the console input device. "CONIN$" is the canonical Windows device
    // path for the console input buffer, analogous to `/dev/tty` on Unix.
    let conin = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .map_err(|e| io::Error::new(e.kind(), format!("failed to open CONIN$: {e}")))?;

    let handle = conin.as_raw_handle();

    // Replace the process's stdin handle so that subsequent calls to
    // GetStdHandle(STD_INPUT_HANDLE) — including crossterm's internal ones
    // for event polling — read from the console rather than the drained pipe.
    const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6u32; // (DWORD)-10
    extern "system" {
        fn SetStdHandle(nStdHandle: u32, hHandle: *mut std::ffi::c_void) -> i32;
    }

    unsafe {
        if SetStdHandle(STD_INPUT_HANDLE, handle) == 0 {
            return Err(io::Error::last_os_error());
        }
    }

    // Leak the File handle to keep CONIN$ open for the process lifetime.
    // SetStdHandle does not take ownership, so without this the handle would
    // be closed when `conin` drops, leaving STD_INPUT_HANDLE dangling.
    std::mem::forget(conin);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    use tracing_subscriber::prelude::*;
    let layer = crate::telemetry::TelemetryLayer;
    let subscriber = tracing_subscriber::registry().with(layer);
    let _ = tracing::subscriber::set_global_default(subscriber);

    let cli = Cli::parse();

    if let Some(shell) = &cli.completions {
        return print_completions(shell);
    }
    if cli.print_man_page {
        return print_man_page();
    }
    if cli.update {
        return crate::update::run_self_update();
    }
    if cli.telemetry_status {
        return print_telemetry_status();
    }

    let stdin_piped = pager::is_piped_stdin();
    match plan_startup(cli.path, cli.language, stdin_piped)? {
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
    // Whenever stdin isn't the terminal (piped/redirected, e.g. `mantis <
    // /dev/null` or `echo x | mantis some/path`), fd 0 can't supply keyboard
    // events, regardless of whether pager mode is showing piped content or a
    // path argument was also given. Checked once up front, before stdin is
    // read to EOF for pager mode — `isatty` reflects the fd itself, not its
    // read position, so this stays accurate either way.
    let stdin_not_tty = pager::is_piped_stdin();

    // On Windows, when stdin is piped, reopen CONIN$ so that crossterm's
    // event system reads from the console input buffer (not the consumed
    // pipe). Must happen before enable_raw_mode so console modes are set on
    // the new handle. Falls back to the (degraded) default keyboard input
    // if CONIN$ can't be opened, mirroring the Unix `for_tty` fallback
    // rather than aborting startup over it.
    #[cfg(windows)]
    if stdin_not_tty {
        let _ = redirect_stdin_to_console();
    }

    enable_raw_mode()?;
    crate::theme::detect_terminal_background();
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    crate::app::set_alternate_scroll(false);

    // Enable kitty keyboard protocol on terminals that support it.
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
    // only the custom raw parser needs the explicit fd override.
    #[cfg(unix)]
    let mut events: Box<dyn EventSource> = if keyboard_enhanced {
        if stdin_not_tty {
            Box::new(event_source::RawEventSource::for_tty())
        } else {
            Box::new(event_source::RawEventSource::new())
        }
    } else {
        Box::new(CrosstermEvents)
    };
    #[cfg(not(unix))]
    let mut events: Box<dyn EventSource> = Box::new(CrosstermEvents);

    #[cfg(not(unix))]
    let _ = keyboard_enhanced;

    run_app(&mut terminal, root, initial, events.as_mut())
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

    // Show the first-run welcome overlay if it has never been dismissed.
    if !crate::session::is_welcome_shown() {
        app.show_welcome = true;
    }

    match initial {
        InitialContent::File(file) => app.open_and_reveal(&file),
        InitialContent::Pager { parsed, language } => app.open_pager_content(parsed, language),
        InitialContent::None => {}
    }
    // Drive tree/git refreshes from filesystem events rather than a blind timer.
    app.watch_root();
    app.install_config_watcher();

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
