use std::io;
use std::path::{Path, PathBuf};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

mod app;
mod command_palette;
mod config;
mod file;
mod git;
mod highlight;
mod markdown;
mod search;
mod selection;
mod theme;
mod tree;
mod ui;

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

fn main() -> anyhow::Result<()> {
    let arg = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .canonicalize()?;

    // If a file is given, root the tree at its parent and open the file.
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

    // Surface a malformed config now that the alternate screen is gone, so the
    // warning lands on the user's normal terminal instead of being painted over.
    if let Some(err) = &app.config_error {
        eprintln!("tv: ignoring invalid config: {err}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_main_{}_{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
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
}
