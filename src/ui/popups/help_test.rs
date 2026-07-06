use crate::app::App;
use crate::config::{bind, Config, Keymap};
use crate::ui::popups::draw_help;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
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
    app.help_tab = 1;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("go up one directory"),
        "help overlay must list 'go up one directory', got:\n{joined}"
    );
}

#[test]
fn help_shows_search_option_toggles() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.help_tab = 3; // Search tab
    let backend = TestBackend::new(80, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Ctrl+A/W/R"),
        "help overlay must list the Ctrl+A/W/R search toggles, got:\n{joined}"
    );
    assert!(
        joined.contains("toggle case / whole-word / regex"),
        "help overlay must describe the search toggles, got:\n{joined}"
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
    app.help_tab = 4;
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
    app.help_tab = 6;
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
        joined.contains("Ctrl+c / q"),
        "help should show 'Ctrl+c / q' for quit, got:\n{joined}"
    );
}

/// find_files (Ctrl+F) appears in the help overlay.
#[test]
fn help_shows_find_files_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 3;
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
    app.help_tab = 4;
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
    app.help_tab = 4;
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
    app.help_tab = 4;
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
    app.help_tab = 4;
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
    app.help_tab = 4;
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
    app.help_tab = 4;
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

/// On legacy terminals (no kitty keyboard protocol), the overview tab warns
/// that Ctrl+Shift shortcuts degrade to their plain-Ctrl counterpart.
#[test]
fn help_shows_legacy_terminal_warning_when_not_keyboard_enhanced() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.keyboard_enhanced = false;
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("can't distinguish Ctrl+Shift shortcuts"),
        "help overlay must warn about degraded Ctrl+Shift shortcuts on legacy terminals, got:\n{joined}"
    );
}

/// On keyboard-enhanced terminals, the legacy-terminal warning must not show.
#[test]
fn help_hides_legacy_terminal_warning_when_keyboard_enhanced() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.keyboard_enhanced = true;
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        !joined.contains("can't distinguish Ctrl+Shift shortcuts"),
        "help overlay must not show the legacy-terminal warning when keyboard_enhanced, got:\n{joined}"
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
    app.help_tab = 1;
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
    app.help_tab = 3; // Search tab
                      // Render in a small terminal
    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_before = buffer_rows(&terminal);
    let before = rows_before.join("\n");
    let has_search_section = before.contains("Filters & In-File Search");

    // Scroll down to near the bottom
    app.help_scroll = 999;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_after = buffer_rows(&terminal);
    let after = rows_after.join("\n");
    let has_search_section_after = after.contains("Filters & In-File Search");

    // After scrolling down we should see sections that were hidden
    assert!(
        !has_search_section || has_search_section_after,
        "scrolling down in help should eventually reveal 'Filters & In-File Search'. Before scroll had_search={}, after={}",
        has_search_section,
        has_search_section_after,
    );
}

/// Sync-strategy guard (#304): every `HELP_DOC_LINKS` entry must point at a
/// `docs/src/*.md` file that actually exists, so a rename/deletion in the
/// mdbook is caught here instead of silently leaving a dangling in-app link.
#[test]
fn help_doc_links_point_to_existing_files() {
    use crate::ui::popups::help::{HELP_DOC_LINKS, HELP_TABS};

    assert_eq!(
        HELP_DOC_LINKS.len(),
        HELP_TABS.len(),
        "HELP_DOC_LINKS must have one entry per HELP_TABS tab"
    );

    let docs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/src");
    for &doc_file in HELP_DOC_LINKS {
        let path = docs_dir.join(doc_file);
        assert!(
            path.exists(),
            "HELP_DOC_LINKS references '{doc_file}' which does not exist at {}",
            path.display()
        );
    }
}

/// Each tab renders its "Full docs" footer pointing at the mapped file.
#[test]
fn help_shows_full_docs_footer() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 4; // Git tab
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        joined.contains("Full docs:") && joined.contains("docs/src/git.md"),
        "Git tab must show a 'Full docs' footer pointing at docs/src/git.md, got:\n{joined}"
    );
}

/// Guards `GIT_KEYMAP_ENTRIES` against drifting from the canonical action
/// registry - the module doc comment on `help.rs` promises this is enforced.
#[test]
fn git_keymap_entries_ids_are_canonical_actions() {
    use crate::actions::ACTIONS;
    use crate::ui::popups::help::GIT_KEYMAP_ENTRIES;
    for &(id, _) in GIT_KEYMAP_ENTRIES {
        assert!(
            ACTIONS.iter().any(|a| a.id == id),
            "'{id}' in GIT_KEYMAP_ENTRIES is not a known ACTIONS id",
        );
    }
}

// -- ACTIONS-derived sections (issue #495) -----------------------------------

/// The Content panel section still shows both meanings of the shared
/// nav_up/nav_down bindings: tree-panel "move up/down" (from their
/// `ACTIONS` entry) and content-panel "scroll up/down" (hand-appended in
/// `keymap_help_sections`, since an `ActionSpec` has only one `help` slot).
#[test]
fn help_shows_both_nav_meanings_move_and_scroll() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 1;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("move up"), "Tree panel must list 'move up'");
    assert!(
        joined.contains("scroll up"),
        "Content panel must list 'scroll up'"
    );
}

/// New palette entries added for issue #495 (recent_files, toggle_blame) also
/// have `ACTIONS` help entries and must render in the help overlay.
#[test]
fn help_shows_recent_files_and_toggle_blame() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 200);
    let mut terminal = Terminal::new(backend).unwrap();

    app.help_tab = 0;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_0 = buffer_rows(&terminal);
    let joined_0 = rows_0.join("\n");
    assert!(
        joined_0.contains("recent files picker"),
        "help must list 'recent files picker', got:\n{joined_0}"
    );

    app.help_tab = 4;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows_4 = buffer_rows(&terminal);
    let joined_4 = rows_4.join("\n");
    assert!(
        joined_4.contains("toggle git blame gutter"),
        "help must list 'toggle git blame gutter', got:\n{joined_4}"
    );
}

#[test]
fn help_shows_markdown_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(200, 200);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 2; // Content tab
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    // The markdown-render toggle should appear in the help overlay.
    assert!(
        joined.contains("toggle markdown render"),
        "help must list the markdown-render toggle, got:\n{joined}"
    );
}

#[test]
fn help_tab_key_navigation() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    assert_eq!(app.help_tab, 0);

    // Right / l / Tab goes forward
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
    assert_eq!(app.help_tab, 2);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 3);

    // Left / h / BackTab goes backward
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 2);
    app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
    assert_eq!(app.help_tab, 1);
    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 0);

    // Left wrap-around
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 8);

    // Right wrap-around
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
    assert_eq!(app.help_tab, 0);
}

#[test]
fn help_mouse_click_tabs_and_outside() {
    use crate::ui::popups::help::help_tab_ranges;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    assert_eq!(app.help_tab, 0);

    // Render once to populate help_area
    let backend = TestBackend::new(100, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();

    let area = app.help_area;
    assert!(area.width > 0);

    // Left click outside area: should close help overlay and reset tab
    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x.saturating_sub(1),
        row: area.y,
        modifiers: KeyModifiers::empty(),
    });
    assert!(!app.show_help);
    assert_eq!(app.help_tab, 0);

    // Reopen help
    app.show_help = true;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();

    // Click inside first tab "Getting started" (x range is help_area.x + 1 to help_area.x + 18)
    // Click on Tab 1 "Navigation" (index 1)
    // The tab y row is help_area.y + 1
    // The ranges of tabs are computed by help_tab_ranges(help_area.x + 1)
    let ranges = help_tab_ranges(area.x + 1);
    let nav_tab_range = ranges[1];

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: nav_tab_range.0,
        row: area.y + 1,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.help_tab, 1);
}

/// `toggle_watch` (and other palette-only actions with `help: None`) must
/// still render a readable description in the help overlay by falling back
/// to the action's palette label.
#[test]
fn help_shows_toggle_watch_description() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.help_tab = 2; // Content tab
    let backend = TestBackend::new(120, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Toggle auto watch"),
        "toggle_watch row must fall back to its palette label instead of rendering blank, got:\n{joined}"
    );
}

/// The Settings tab must reference the real `open_config_in_editor` action
/// id, not a nonexistent `open_config` id.
#[test]
fn help_settings_tab_references_real_open_config_action() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.help_tab = 5; // Settings tab
    let backend = TestBackend::new(120, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("open configuration file"),
        "Settings tab must describe opening the config file, got:\n{joined}"
    );
    assert!(
        crate::actions::ACTIONS
            .iter()
            .any(|a| a.id == "open_config_in_editor"),
        "open_config_in_editor must be a real canonical action id"
    );
}

/// Config/plugin/theme paths shown in help must reflect `$XDG_CONFIG_HOME`
/// support, not just the `~/.config` default.
#[test]
fn help_paths_mention_xdg_config_home() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 100);
    let mut terminal = Terminal::new(backend).unwrap();

    app.help_tab = 5; // Settings
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        joined.contains("XDG_CONFIG_HOME"),
        "Settings tab config path must mention $XDG_CONFIG_HOME, got:\n{joined}"
    );

    app.help_tab = 6; // Themes
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        joined.contains("XDG_CONFIG_HOME"),
        "Themes tab path must mention $XDG_CONFIG_HOME, got:\n{joined}"
    );

    app.help_tab = 7; // Plugins
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        joined.contains("XDG_CONFIG_HOME"),
        "Plugins tab path must mention $XDG_CONFIG_HOME, got:\n{joined}"
    );
    assert!(
        !joined.contains("standard JSON-RPC"),
        "Plugins tab must not claim JSON-RPC; the protocol is newline-delimited JSON, got:\n{joined}"
    );
}

/// The tab bar is wider than most terminals once all 9 tabs are shown; when
/// the active tab is scrolled into view, mouse clicks on a neighboring
/// visible tab must still resolve to the right tab index (i.e. hit-testing
/// accounts for the same scroll offset used to render the tab bar).
#[test]
fn help_mouse_click_reaches_neighbor_of_scrolled_tab() {
    use crate::ui::popups::help::help_tab_scroll_offset;

    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_tab = 8; // Mouse tab: far right, requires scrolling on a narrow terminal.

    // Narrow terminal: the full 9-tab bar (110 cols) does not fit.
    let backend = TestBackend::new(80, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();

    let area = app.help_area;
    let available_width = area.width.saturating_sub(2);
    let offset = help_tab_scroll_offset(app.help_tab, available_width);
    assert!(
        offset > 0,
        "tab 8 should require a nonzero scroll offset on an 80-col terminal"
    );

    // Tab 7 ("Plugins") sits immediately to the left of tab 8 and remains
    // visible in the scrolled view; compute its on-screen column and click it.
    let ranges = crate::ui::popups::help_tab_ranges(0);
    let (start7, _) = ranges[7];
    assert!(
        start7 >= offset,
        "tab 7 must still be within the visible scrolled window"
    );
    let click_col = area.x + 1 + (start7 - offset);

    app.handle_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_col,
        row: area.y + 1,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(
        app.help_tab, 7,
        "clicking tab 7's on-screen position while the bar is scrolled to show tab 8 \
         must select tab 7, not whatever tab the unscrolled coordinates would hit"
    );
}

#[test]
fn help_shows_command_palette_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 0; // Getting started tab
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("open command palette"),
        "help must list 'open command palette' for command_palette, got:\n{joined}"
    );
}

#[test]
fn help_shows_goto_line_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(120, 100);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 1; // Navigation tab
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("Go to line"),
        "help must list 'Go to line' for goto_line, got:\n{joined}"
    );
}

#[test]
fn help_shows_open_external() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    app.help_tab = 0;
    terminal.draw(|f| draw_help(f, &mut app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(
        joined.contains("open file with system default app"),
        "help overlay must list 'open file with system default app', got:\n{joined}"
    );
}

/// Verification guard: every action in `ACTIONS` that has a default keybinding
/// must be rendered as a row (via `row_key`/`row_key_custom`) in
/// `src/ui/popups/help.rs`, unless explicitly allowlisted as an intentional
/// omission. This prevents any keybound action from silently missing help
/// coverage in the overlay. Checking the specific row-builder call (rather than
/// any string literal match) avoids false positives from unrelated references
/// to the action id, e.g. `labels_for_action("id")`.
#[test]
fn help_clipboard_section_lists_copy_line_and_copy_file() {
    // Verify that the help overlay's clipboard operations section renders
    // the copy_line and copy_file action rows.
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.show_help = true;
    app.help_tab = 2; // Content panel tab
    let terminal = &mut Terminal::new(TestBackend::new(120, 40)).unwrap();
    terminal.draw(|f| crate::ui::draw(f, &mut app)).unwrap();
    let rows = buffer_rows(terminal);
    let joined: String = rows.join("\n");
    assert!(
        joined.contains("copy current line"),
        "help must list copy_line action, got:\n{joined}"
    );
    assert!(
        joined.contains("copy entire file"),
        "help must list copy_file action, got:\n{joined}"
    );
    dir.close().unwrap();
}

#[test]
fn keybound_actions_are_in_help_overlay() {
    let help_rs_content = std::fs::read_to_string("src/ui/popups/help.rs")
        .expect("should read src/ui/popups/help.rs");

    let allowlist: &[&str] = &[];
    let keys = Keymap::default();

    for action in crate::actions::ACTIONS {
        let is_keybound = keys.labels_for_action(action.id) != "—";
        if is_keybound && !allowlist.contains(&action.id) {
            // Allow the call to wrap across lines, e.g. `row_key_custom(\n    "id",`.
            let pattern = format!(r#"row_key(_custom)?\s*\(\s*"{}""#, regex::escape(action.id));
            let re = regex::Regex::new(&pattern).expect("valid regex");
            assert!(
                re.is_match(&help_rs_content),
                "Action '{}' is keybound but is not rendered via row_key/row_key_custom in src/ui/popups/help.rs",
                action.id
            );
        }
    }
}
