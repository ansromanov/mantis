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
mod diff;
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
    // Drive tree/git refreshes from filesystem events rather than a blind timer.
    app.watch_root();

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
#[path = "main_test.rs"]
mod tests;
