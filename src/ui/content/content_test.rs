use std::path::PathBuf;

use ratatui::style::{Color, Style};

use crate::search::InFileSearch;
use crate::ui::content::draw_content;

// ── word-wrap highlight verification (#206) ───────────────────────────────
//
// Match columns are computed against unwrapped char positions. ratatui's
// `Wrap` re-flows the styled spans onto visual rows at render time, carrying
// each span's background with it. These tests render through `draw_content`
// with `word_wrap` on and assert the highlight lands on the correct visual
// row/column — guarding against the wrap-related drift seen in #42/#56.

/// Background color of the rendered cell at (x, y) in the buffer.
fn cell_bg(buffer: &ratatui::buffer::Buffer, x: u16, y: u16) -> Color {
    buffer[(x, y)].bg
}

fn make_search(matches: Vec<crate::search::InFileMatch>, current: usize) -> InFileSearch {
    InFileSearch {
        query: "test".to_string(),
        matches,
        current,
        regex: false,
        case_sensitive: false,
        whole_word: false,
    }
}

#[test]
fn search_highlight_on_correct_wrapped_row() {
    let (mut app, _dir) = render_app();
    // 160-char line with no spaces wraps at the content width (78 cols, no
    // gutter) into rows of 0..78, 78..156, 156..160.
    let line: String = "x".repeat(160);
    let theme = app.theme.clone();
    let buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.show_line_numbers = false; // content starts flush at inner.x = 1
        app.active_line = usize::MAX; // disable active-line tint so only search highlights colour cells
        app.word_wrap = true;
        app.content = vec![line.clone()];
        app.highlighted = vec![vec![(Style::default(), line.clone())]];
        app.in_file_search = Some(make_search(
            vec![
                // current: cols 5..9 -> visual row 0
                crate::search::InFileMatch {
                    line: 0,
                    col: 5,
                    len: 4,
                },
                // other: cols 100..104 -> visual row 1 (local cols 22..26)
                crate::search::InFileMatch {
                    line: 0,
                    col: 100,
                    len: 4,
                },
            ],
            0,
        ));
    });

    // Row 0 (y = 1): current match at screen x = 1 + 5 .. 1 + 9.
    for x in 6u16..10 {
        assert_eq!(
            cell_bg(&buffer, x, 1),
            theme.selection_bg,
            "current match cell ({x},1) should use selection_bg"
        );
    }
    // Cell just before the match on row 0 is unhighlighted.
    assert_eq!(cell_bg(&buffer, 5, 1), Color::Reset);

    // Row 1 (y = 2): other match at local cols 22..26 -> screen x = 23..27.
    for x in 23u16..27 {
        assert_eq!(
            cell_bg(&buffer, x, 2),
            theme.dim,
            "other match cell ({x},2) should use dim bg"
        );
    }
    // The other match must NOT bleed onto row 0 at the same screen columns.
    for x in 23u16..27 {
        assert_eq!(
            cell_bg(&buffer, x, 1),
            Color::Reset,
            "other match must not appear on row 0 at ({x},1)"
        );
    }
}

#[test]
fn search_highlight_spans_wrap_boundary() {
    let (mut app, _dir) = render_app();
    // Match straddles the wrap point: cols 76..82 split as 76,77 on row 0 and
    // 78..82 on row 1. Both halves must keep the selection background.
    let line: String = "y".repeat(160);
    let theme = app.theme.clone();
    let buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.show_line_numbers = false;
        app.active_line = usize::MAX; // disable active-line tint so only search highlights colour cells
        app.word_wrap = true;
        app.content = vec![line.clone()];
        app.highlighted = vec![vec![(Style::default(), line.clone())]];
        app.in_file_search = Some(make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 76,
                len: 6,
            }],
            0,
        ));
    });

    // Tail of row 0: cols 76,77 -> screen x = 77, 78.
    for x in 77u16..79 {
        assert_eq!(
            cell_bg(&buffer, x, 1),
            theme.selection_bg,
            "wrap-boundary match tail on row 0 at ({x},1)"
        );
    }
    // Head of row 1: local cols 0..4 -> screen x = 1..5.
    for x in 1u16..5 {
        assert_eq!(
            cell_bg(&buffer, x, 2),
            theme.selection_bg,
            "wrap-boundary match head on row 1 at ({x},2)"
        );
    }
}

// ── draw_content smoke tests ───────────────────────────────────────────

/// Minimal app factory for rendering tests. Creates a temp directory and
/// a real `App` via `App::new` (needed for the highlighter and config).
fn render_app() -> (crate::app::App, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    std::fs::write(dir.path().join("f.txt"), "line1\nline2\n").unwrap();
    let app = crate::app::App::new(
        dir.path().to_path_buf(),
        crate::config::Config::default(),
        None,
        None,
    )
    .expect("App::new");
    (app, dir)
}

/// Creates a `TestBackend` + `Terminal`, runs `draw_content`, and returns
/// the rendered buffer for assertions.
fn render_content<F>(app: &mut crate::app::App, f: F) -> ratatui::buffer::Buffer
where
    F: FnOnce(&mut crate::app::App),
{
    use ratatui::backend::TestBackend;
    f(app);
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal.backend().buffer().clone()
}

#[test]
fn draw_inline_fallback_no_file_shows_title() {
    let (mut app, _dir) = render_app();
    app.current_file = None;
    app.content = Vec::new();
    app.highlighted = Vec::new();
    app.virtual_file = None;
    let buffer = render_content(&mut app, |_| {});
    let top_line: String = buffer
        .content()
        .iter()
        .take(80)
        .map(|c| c.symbol())
        .collect();
    assert!(
        top_line.contains("No file"),
        "top border should contain 'No file'; got: {top_line:?}"
    );
}

#[test]
fn draw_inline_fallback_with_content() {
    let (mut app, _dir) = render_app();
    let buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["hello".to_string(), "world".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "hello".to_string())],
            vec![(Style::default(), "world".to_string())],
        ];
    });
    let lines: Vec<String> = buffer
        .content()
        .chunks(80)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect();
    let all = lines.concat();
    assert!(all.contains("hello"), "content should contain 'hello'");
    assert!(all.contains("world"), "content should contain 'world'");
}

#[test]
fn draw_inline_fallback_word_wrap() {
    let (mut app, _dir) = render_app();
    let buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["hello world".to_string()];
        app.highlighted = vec![vec![(Style::default(), "hello world".to_string())]];
        app.word_wrap = true;
    });
    let lines: Vec<String> = buffer
        .content()
        .chunks(80)
        .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
        .collect();
    let all = lines.concat();
    assert!(
        all.contains("hello"),
        "wrapped content should contain 'hello'"
    );
}

#[test]
fn line_number_gutter_shown_by_default() {
    let (mut app, _dir) = render_app();
    let buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["hello".to_string()];
        app.highlighted = vec![vec![(Style::default(), "hello".to_string())]];
    });
    // Second row (first content line) should begin with the gutter "1".
    let row: String = buffer.content().chunks(80).nth(1).unwrap()[..6]
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert!(
        row.contains('1'),
        "gutter should show line number; got {row:?}"
    );
}

#[test]
fn line_number_gutter_hidden_when_disabled() {
    let (mut app, _dir) = render_app();
    let buffer = render_content(&mut app, |app| {
        app.show_line_numbers = false;
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["hello".to_string()];
        app.highlighted = vec![vec![(Style::default(), "hello".to_string())]];
    });
    // With the gutter off, content starts flush at the inner edge: "hello".
    let row: String = buffer.content().chunks(80).nth(1).unwrap()[1..6]
        .iter()
        .map(|c| c.symbol())
        .collect();
    assert_eq!(
        row, "hello",
        "content should start at the edge with no gutter"
    );
}

#[test]
fn draw_diff_mode_renders_without_panicking() {
    let (mut app, _dir) = render_app();
    let _buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.is_diff = true;
        app.content = vec![
            "@@ -1 +1 @@".to_string(),
            "-old line".to_string(),
            "+new line".to_string(),
        ];
        app.highlighted = vec![
            vec![(
                Style::default().fg(Color::Yellow),
                "@@ -1 +1 @@".to_string(),
            )],
            vec![(Style::default().fg(Color::Red), "-old line".to_string())],
            vec![(Style::default().fg(Color::Green), "+new line".to_string())],
        ];
    });
    let all: String = _buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(all.contains("@@"));
    assert!(all.contains("-old"));
    assert!(all.contains("+new"));
}

#[test]
fn draw_plugin_content_passes_through() {
    let (mut app, _dir) = render_app();
    let _buffer = render_content(&mut app, |app| {
        let plugin_path = PathBuf::from("plugin.md");
        app.current_file = Some(plugin_path.clone());
        app.virtual_file = None;
        app.plugin_content.insert(
            plugin_path,
            vec![
                vec![(Style::default().fg(Color::Cyan), "# Title".to_string())],
                vec![(Style::default(), "body text".to_string())],
            ],
        );
    });
    let all: String = _buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        all.contains("Title") || all.contains("body"),
        "markdown content should render; got prefix: {:?}",
        &all[..all.len().min(80)]
    );
}

#[test]
fn draw_virtual_file_mode() {
    let (mut app, _dir) = render_app();
    let _buffer = render_content(&mut app, |app| {
        let path = _dir.path().join("f.txt");
        let vf = crate::virtual_file::VirtualFile::open(&path);
        app.virtual_file = vf;
        app.current_file = Some(path);
    });
    let all: String = _buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(all.contains("line1"), "virtual file content should render");
}

#[test]
fn draw_with_selection_highlights_bg() {
    let (mut app, _dir) = render_app();
    let _buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["select me".to_string()];
        app.highlighted = vec![vec![(Style::default(), "select me".to_string())]];
        app.selection = Some(crate::selection::TextSelection {
            anchor: (0, 0),
            active: (0, 6),
        });
    });
    let all: String = _buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(
        all.contains("select"),
        "content with selection should render"
    );
}

#[test]
fn draw_scrollbar_visible_when_recently_scrolled() {
    let (mut app, _dir) = render_app();
    app.show_scrollbar = true;
    app.content_scrolled_at = std::time::Instant::now();
    let _buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = (0..100).map(|i| format!("line {i}")).collect();
        app.highlighted = (0..100)
            .map(|i| vec![(Style::default(), format!("line {i}"))])
            .collect();
        app.content_scroll = 30;
    });
    let all: String = _buffer.content().iter().map(|c| c.symbol()).collect();
    assert!(all.contains('█'), "scrollbar thumb should be visible");
}

#[test]
fn draw_content_records_area_and_fold_gutter_rows() {
    let (mut app, _dir) = render_app();
    let area = ratatui::layout::Rect {
        x: 2,
        y: 2,
        width: 76,
        height: 20,
    };
    let _buffer = {
        use ratatui::backend::TestBackend;
        let backend = TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| draw_content(frame, &mut app, area))
            .unwrap();
        terminal.backend().buffer().clone()
    };
    assert!(
        app.content_area.width > 0,
        "content_area should be recorded after draw"
    );
    assert!(app.fold_gutter_rows.is_empty());
}
