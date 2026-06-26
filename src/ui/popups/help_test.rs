use crate::app::App;
use crate::config::Config;
use crate::ui::popups::draw_help;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn make_app(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn buffer_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect()
}

#[test]
fn help_shows_backspace_tree_up_dir() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("go up one directory"),
        "help overlay must list 'go up one directory', got:\n{joined}"
    );
}

#[test]
fn help_shows_dot_for_toggle_hidden() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("toggle hidden files"),
        "help overlay must list 'toggle hidden files', got:\n{joined}"
    );
}

#[test]
fn help_shows_f_for_git_flat_toggle() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("toggle git flat/tree view"),
        "help overlay must list 'toggle git flat/tree view', got:\n{joined}"
    );
}
