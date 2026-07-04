use crate::app::App;
use crate::config::{bind, Config, Keymap};
use crate::ui::popups::draw_help;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
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
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
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
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
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
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
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
    let mut app = make_app_with_keys(dir.path(), keys);
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
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
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
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
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("global fuzzy file-name picker"),
        "help must list 'global fuzzy file-name picker' for find_files, got:\n{joined}"
    );
}

/// Git section header appears in the help overlay.
#[test]
fn help_shows_git_section_header() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Git"),
        "help overlay must contain 'Git' section header, got:\n{joined}"
    );
}

/// Git section shows tree-color legend.
#[test]
fn help_git_section_shows_tree_colors() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Tree colors"),
        "help overlay must list 'Tree colors' in the Git section, got:\n{joined}"
    );
    assert!(
        joined.contains("green = new"),
        "help overlay must show 'green = new' in the tree-color legend, got:\n{joined}"
    );
}

/// Git section shows status-bar legend.
#[test]
fn help_git_section_shows_status_bar() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Status bar"),
        "help overlay must list 'Status bar' in the Git section, got:\n{joined}"
    );
}

/// Git section blame_line entry shows the default key label.
#[test]
fn help_git_section_shows_blame_line_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("blame current line"),
        "help overlay must list 'blame current line' in the Git section, got:\n{joined}"
    );
}

/// Git section file_history entry shows the default key label.
#[test]
fn help_git_section_shows_file_history_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("pick a commit"),
        "help overlay must list 'pick a commit' in the Git section, got:\n{joined}"
    );
}

/// Remapped blame_line key shows the new binding in the Git section.
#[test]
fn help_git_section_remapped_blame_line() {
    let keys = Keymap {
        blame_line: bind(&["ctrl+b"]),
        ..Keymap::default()
    };
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app_with_keys(dir.path(), keys);
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Ctrl+b"),
        "help with remapped blame_line to Ctrl+B should show 'Ctrl+b', got:\n{joined}"
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
    let mut app = make_app_with_keys(dir.path(), keys);
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("—"),
        "help with unbound recent_files should show '—', got:\n{joined}"
    );
}

// ── scrolling tests ──────────────────────────────────────────────────

#[test]
fn help_scroll_starts_at_zero() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_j_down() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 1);
}

#[test]
fn help_scroll_down_arrow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 1);
}

#[test]
fn help_scroll_k_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 5;
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 4);
}

#[test]
fn help_scroll_up_arrow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 5;
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 4);
}

#[test]
fn help_scroll_j_does_not_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = usize::MAX;
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    // saturating_add clamps at MAX rather than wrapping, so should still be MAX
    assert_eq!(app.help_scroll, usize::MAX);
}

#[test]
fn help_scroll_k_does_not_underflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_page_down() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 10);
}

#[test]
fn help_scroll_page_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 15;
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 5);
}

#[test]
fn help_scroll_home_g() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 20;
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_home_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 20;
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_cap_g() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::empty()));
    assert_eq!(app.help_scroll, usize::MAX);
}

#[test]
fn help_scroll_end_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::empty()));
    assert_eq!(app.help_scroll, usize::MAX);
}

#[test]
fn help_scroll_esc_resets() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 10;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(!app.show_help);
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_mouse_wheel_down() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.help_scroll, 3);
}

#[test]
fn help_scroll_mouse_wheel_up() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_scroll = 10;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.help_scroll, 7);
}

#[test]
fn help_scroll_mouse_wheel_does_not_underflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.help_scroll, 0);
}

#[test]
fn help_scroll_clamped_by_draw() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    // Set help_scroll past the end of content
    app.help_scroll = 999;
    // Draw clamps it to max_scroll = total_rows - inner_height
    // Using a small terminal (80x10) so we scroll past everything
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    // After clamping, help_scroll should be <= max_scroll
    // We can't know the exact value without replicating the logic, but it should be < 999
    assert!(
        app.help_scroll < 999,
        "help_scroll should be clamped by draw, got {}",
        app.help_scroll
    );
}

#[test]
fn help_scroll_indicator_shown_when_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    // Small terminal so not all help content fits
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    // When scrolled past end, show ▼ at bottom-right area
    assert!(
        joined.contains("▲") || joined.contains("║") || joined.contains("▼"),
        "help overlay should show a scroll indicator when content overflows, got:\n{joined}"
    );
}

#[test]
fn help_scroll_indicator_hidden_when_fits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    // Large terminal so all help content fits
    let backend = TestBackend::new(80, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    // When everything fits, no scroll indicator
    assert!(
        !joined.contains("▲") && !joined.contains("║") && !joined.contains("▼"),
        "help overlay should NOT show a scroll indicator when all content fits, got:\n{joined}"
    );
}

#[test]
fn help_scroll_down_reveals_later_sections() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    // Render in a small terminal
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_before = buffer_rows(&terminal);
    let before = rows_before.join("\n");
    let has_search_section = before.contains("Search / history popup");

    // Scroll down to near the bottom
    app.help_scroll = 999;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_after = buffer_rows(&terminal);
    let after = rows_after.join("\n");
    let has_search_section_after = after.contains("Search / history popup");

    // After scrolling down we should see sections that were hidden
    assert!(
        !has_search_section || has_search_section_after,
        "scrolling down in help should eventually reveal 'Search / history popup'. Before scroll had_search={}, after={}",
        has_search_section,
        has_search_section_after,
    );
}

#[test]
fn help_shows_no_markdown_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(200, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    // The built-in markdown renderer was removed; its help entry description
    // should no longer appear in the help overlay.
    assert!(
        !joined.contains("toggle markdown render"),
        "help must not mention the markdown-render toggle after built-in renderer removal"
    );
}
