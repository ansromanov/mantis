use std::io;
use std::path::PathBuf;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

mod app;
mod config;
mod file;
mod git;
mod highlight;
mod markdown;
mod tree;
mod ui;

fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()?;

    // If a file is given, root the tree at its parent and open the file.
    let (root, file) = if arg.is_file() {
        let parent = arg
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        (parent, Some(arg))
    } else {
        (arg, None)
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let cfg = config::load(&root);
    let mut app = app::App::new(root, cfg)?;
    if let Some(file) = file {
        app.open_and_reveal(&file);
    }

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        if event::poll(std::time::Duration::from_millis(16))? {
            match event::read()? {
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

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
