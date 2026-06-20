use ratatui::layout::Rect;

use crate::ui::popups::util::centered_rect;
use crate::ui::popups::{
    draw_about, draw_blame_panel, draw_command_palette, draw_help, draw_history,
    draw_in_file_search, draw_recent, draw_search, draw_theme,
};

#[test]
fn centered_rect_returns_inner_rectangle() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 100,
    };
    let result = centered_rect(50, 50, area);
    assert_eq!(result.width, 50);
    assert_eq!(result.height, 50);
    assert_eq!(result.x, 25);
    assert_eq!(result.y, 25);
}

#[test]
fn centered_rect_full_size() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    let result = centered_rect(100, 100, area);
    assert_eq!(result.width, 80);
    assert_eq!(result.height, 24);
    assert_eq!(result.x, 0);
    assert_eq!(result.y, 0);
}

#[test]
fn centered_rect_narrow_and_short() {
    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 24,
    };
    let result = centered_rect(20, 20, area);
    assert_eq!(result.width, 16);
    assert_eq!(result.height, 4);
    assert_eq!(result.x, 32);
    assert_eq!(result.y, 10);
}

#[test]
fn centered_rect_non_zero_origin() {
    let area = Rect {
        x: 10,
        y: 5,
        width: 80,
        height: 40,
    };
    let result = centered_rect(50, 50, area);
    assert_eq!(result.width, 40);
    assert_eq!(result.height, 20);
    assert_eq!(result.x, 10 + 20);
    assert_eq!(result.y, 5 + 10);
}

// ── draw_search ─────────────────────────────────────────────────────────

use crate::app::App;
use crate::config::Config;
use crate::search::InFileSearch;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn make_app(root: &std::path::Path) -> App {
    let cfg = Config {
        git_status: false,
        ..Config::default()
    };
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
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
fn draw_search_files_mode() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("alpha.txt"), "").unwrap();
    std::fs::write(dir.path().join("beta.txt"), "").unwrap();
    let mut app = make_app(dir.path());
    app.search = Some(crate::search::SearchState::new(dir.path(), false, true, 0));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Search: Files"));
    assert!(joined.contains("alpha.txt"));
    assert!(joined.contains("beta.txt"));
}

#[test]
fn draw_search_files_filtered() {
    let dir = tempfile::tempdir().unwrap();
    let one = dir.path().join("111111_document_only.txt");
    let two = dir.path().join("222222_document_only.txt");
    std::fs::write(&one, "").unwrap();
    std::fs::write(&two, "").unwrap();
    let mut app = make_app(dir.path());
    app.search = Some(crate::search::SearchState::new(dir.path(), false, true, 0));
    for c in "111111".chars() {
        app.search.as_mut().unwrap().push(c);
    }

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("111111_document_only.txt"));
    assert!(!joined.contains("222222_document_only.txt"));
}

#[test]
fn draw_search_content_mode() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("hello.txt"), "hello world\nfoo bar\n").unwrap();
    std::fs::write(dir.path().join("other.txt"), "no match").unwrap();
    let mut app = make_app(dir.path());
    let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
    search.toggle_mode();
    search.push('h');
    search.push('e');
    search.refresh_now();
    app.search = Some(search);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Search: Content"));
    assert!(joined.contains("hello.txt"));
    assert!(joined.contains("hello world"));
}

#[test]
fn draw_search_content_short_query_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
    search.toggle_mode();
    search.push('x');
    app.search = Some(search);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("2+ chars"));
}

#[test]
fn draw_search_content_query_two_chars_hides_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
    search.toggle_mode();
    search.push('x');
    search.push('y');
    app.search = Some(search);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Search: Content"));
    assert!(!joined.is_empty());
}

#[test]
fn draw_search_select_highlight() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "").unwrap();
    std::fs::write(dir.path().join("b.txt"), "").unwrap();
    let mut app = make_app(dir.path());
    let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
    search.selected = 1;
    app.search = Some(search);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_search(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("b.txt"));
}

// ── draw_history ────────────────────────────────────────────────────────

#[test]
fn draw_history_with_commits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.history = Some(crate::search::HistoryState::new(
        dir.path().join("test.txt"),
        vec![
            crate::git::Commit {
                hash: "abc123def456".into(),
                short: "abc123".into(),
                date: "2024-01-15".into(),
                subject: "fix critical bug".into(),
            },
            crate::git::Commit {
                hash: "def789abc012".into(),
                short: "def789".into(),
                date: "2024-01-14".into(),
                subject: "add new feature".into(),
            },
        ],
    ));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_history(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("History:"));
    assert!(joined.contains("abc123"));
    assert!(joined.contains("def789"));
    assert!(joined.contains("fix critical bug"));
    assert!(joined.contains("add new feature"));
}

#[test]
fn draw_history_empty_commits() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.history = Some(crate::search::HistoryState::new(
        dir.path().join("test.txt"),
        vec![],
    ));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_history(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("History:"));
}

// ── draw_theme ──────────────────────────────────────────────────────────

#[test]
fn draw_theme_with_presets() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.theme_picker = Some(crate::search::ThemePicker::default());

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_theme(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Theme"));
    assert!(joined.contains("default"));
}

#[test]
fn draw_theme_with_filter() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.theme_picker = Some(crate::search::ThemePicker::default());
    if let Some(ref mut p) = app.theme_picker {
        p.push('m');
    }

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_theme(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("> m"));
}

// ── draw_help ───────────────────────────────────────────────────────────

#[test]
fn draw_help_all_sections() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());

    let backend = TestBackend::new(80, 75);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Help"));
    assert!(joined.contains("Global"));
    assert!(joined.contains("Tree panel"));
    assert!(joined.contains("Content panel"));
    assert!(joined.contains("In-file search"));
    assert!(joined.contains("Search / history popup"));
    assert!(joined.contains("toggle this help"));
    assert!(joined.contains("fuzzy file search"));
    assert!(joined.contains("toggle word wrap"));
}

// ── draw_in_file_search ─────────────────────────────────────────────────

#[test]
fn draw_in_file_search_with_matches() {
    use crate::search::InFileMatch;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.in_file_search = Some(InFileSearch {
        query: "hello".into(),
        matches: vec![InFileMatch {
            line: 0,
            col: 0,
            len: 5,
        }],
        current: 0,
    });

    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 30,
    };
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, area))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("/hello"));
    assert!(joined.contains("(1/1)"));
}

#[test]
fn draw_in_file_search_no_matches() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.in_file_search = Some(InFileSearch {
        query: "zzz".into(),
        matches: vec![],
        current: 0,
    });

    let area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 30,
    };
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, area))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("/zzz"));
    assert!(joined.contains("(0/0)"));
}

#[test]
fn draw_in_file_search_narrow_area_returns_early() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.in_file_search = Some(InFileSearch {
        query: "x".into(),
        matches: vec![],
        current: 0,
    });

    let area = Rect {
        x: 0,
        y: 0,
        width: 3,
        height: 30,
    };
    let backend = TestBackend::new(3, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_in_file_search(f, &mut app, area))
        .unwrap();
}

// ── draw_command_palette ────────────────────────────────────────────────

#[test]
fn draw_command_palette_all_commands() {
    use crate::command_palette::CommandPalette;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.command_palette = Some(CommandPalette::default());

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_command_palette(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Commands"));
    assert!(joined.contains("Toggle help"));
    assert!(joined.contains("Toggle hidden files"));
}

#[test]
fn draw_command_palette_filtered() {
    use crate::command_palette::CommandPalette;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let mut cp = CommandPalette::default();
    cp.push('w');
    app.command_palette = Some(cp);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_command_palette(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Toggle word wrap"));
    assert!(!joined.contains("Toggle help"));
}

#[test]
fn draw_command_palette_none_returns_early() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.command_palette = None;

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_command_palette(f, &mut app, f.area()))
        .unwrap();
}

// ── draw_blame_panel ────────────────────────────────────────────────────

#[test]
fn draw_blame_panel_untracked_file_shows_message() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.txt"), "one\ntwo\nthree\n").unwrap();
    let mut app = make_app(dir.path());
    app.open_file(&dir.path().join("a.txt"));
    app.visual_line = Some(crate::selection::VisualLine::new(0));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_blame_panel(f, &app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Blame:"));
    assert!(joined.contains("No blame available"));
}

#[test]
fn draw_blame_panel_noop_without_visual_line() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_blame_panel(f, &app, f.area()))
        .unwrap();
}

// ── draw_about ──────────────────────────────────────────────────────────

#[test]
fn draw_about_shows_version() {
    let dir = tempfile::tempdir().unwrap();
    let app = make_app(dir.path());

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_about(f, &app, f.area())).unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("About"));
    assert!(joined.contains("Version:"));
    assert!(joined.contains("GPL-3.0"));
    assert!(joined.contains("tree viewer"));
}

// ── draw_recent ─────────────────────────────────────────────────────────

#[test]
fn draw_recent_shows_paths_and_records_geometry() {
    use crate::search::RecentFilesState;

    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("alpha.rs");
    let b = dir.path().join("beta.rs");
    std::fs::write(&a, "").unwrap();
    std::fs::write(&b, "").unwrap();
    let mut app = make_app(dir.path());
    app.recent_files = Some(RecentFilesState::new(vec![a, b]));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_recent(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Recent files"));
    assert!(joined.contains("alpha.rs"));
    assert!(joined.contains("beta.rs"));
    // geometry must be recorded for mouse hit-testing
    assert!(app.recent_area.width > 0);
    assert!(app.recent_area.height > 0);
}
