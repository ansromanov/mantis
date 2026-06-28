use crate::app::App;
use crate::config::{bind, Config, Keymap};
use crate::ui::popups::draw_help;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn make_app(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn make_app_with_keys(root: &std::path::Path, keys: Keymap) -> App {
    let config = Config {
        keys,
        ..Config::default()
    };
    App::new(root.to_path_buf(), config, None, None).unwrap()
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

/// When a key is remapped the help overlay shows the new binding.
#[test]
fn help_remapped_key_shows_new_binding() {
    let keys = Keymap {
        theme_picker: bind(&["ctrl+t"]),
        ..Keymap::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let app = make_app_with_keys(dir.path(), keys);
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Ctrl+t"),
        "help with remapped theme_picker to Ctrl+T should show 'Ctrl+t', got:\n{joined}"
    );
}

/// Multi-binding actions show all bindings joined by ` / `.
#[test]
fn help_multi_binding_shows_joined() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("q / Ctrl+c"),
        "help should show 'q / Ctrl+c' for quit, got:\n{joined}"
    );
}

/// find_files (Ctrl+F) appears in the help overlay.
#[test]
fn help_shows_find_files_entry() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("global fuzzy file-name picker"),
        "help must list 'global fuzzy file-name picker' for find_files, got:\n{joined}"
    );
}

/// Unbound actions show `—` instead of a key label.
#[test]
fn help_unbound_action_shows_dash() {
    let keys = Keymap {
        recent_files: vec![],
        ..Keymap::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let app = make_app_with_keys(dir.path(), keys);
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("—"),
        "help with unbound recent_files should show '—', got:\n{joined}"
    );
}
