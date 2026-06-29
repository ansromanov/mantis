//! Tests for the virtual-file and inline-fallback render arms (see
//! `draw_text.rs`). Both are exercised through `draw_content` since the
//! render functions take the app + layout and are not meant to be called in
//! isolation.

use ratatui::backend::TestBackend;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::ui::content::draw_content;

fn render_app() -> (crate::app::App, tempfile::TempDir) {
    let dir = tempfile::TempDir::new().expect("temp dir");
    std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
    let app = crate::app::App::new(
        dir.path().to_path_buf(),
        crate::config::Config::default(),
        None,
        None,
    )
    .expect("App::new");
    (app, dir)
}

fn render<F: FnOnce(&mut crate::app::App)>(app: &mut crate::app::App, f: F) -> String {
    f(app);
    let backend = TestBackend::new(80, 24);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_content(frame, app, frame.area()))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn render_virtual_file_shows_line_numbers_and_text() {
    let (mut app, dir) = render_app();
    let path = dir.path().join("f.txt");
    app.open_file(&path);
    assert!(
        app.virtual_file.is_some(),
        "plain file should mmap as virtual"
    );

    let screen = render(&mut app, |_| {});
    assert!(
        screen.contains("line1"),
        "content should render; got: {screen:?}"
    );
    assert!(screen.contains('1'), "line-number gutter should render");
}

#[test]
fn render_inline_fallback_renders_loaded_content() {
    let (mut app, _dir) = render_app();
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["alpha".to_string(), "beta".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "alpha".to_string())],
            vec![(Style::default(), "beta".to_string())],
        ];
    });
    assert!(screen.contains("alpha"), "got: {screen:?}");
    assert!(screen.contains("beta"), "got: {screen:?}");
}

#[test]
fn render_inline_fallback_empty_does_not_panic() {
    let (mut app, _dir) = render_app();
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = Vec::new();
        app.highlighted = Vec::new();
    });
    assert!(!screen.is_empty());
}

#[test]
fn highlight_cache_populated_after_first_render() {
    let (mut app, dir) = render_app();
    app.open_file(&dir.path().join("f.txt"));
    assert!(app.virtual_file.is_some());
    assert!(
        app.content_highlight_cache.borrow().is_none(),
        "cache must be empty before any render"
    );
    render(&mut app, |_| {});
    assert!(
        app.content_highlight_cache.borrow().is_some(),
        "cache must be populated after rendering a virtual file"
    );
}

#[test]
fn highlight_cache_key_stable_on_identical_render() {
    let (mut app, dir) = render_app();
    app.open_file(&dir.path().join("f.txt"));
    render(&mut app, |_| {});
    let key_after_first = app
        .content_highlight_cache
        .borrow()
        .as_ref()
        .map(|(k, _)| (k.scroll, k.visible_end));
    render(&mut app, |_| {});
    let key_after_second = app
        .content_highlight_cache
        .borrow()
        .as_ref()
        .map(|(k, _)| (k.scroll, k.visible_end));
    assert_eq!(
        key_after_first, key_after_second,
        "cache key must be stable across identical renders"
    );
}

#[test]
fn blame_col_width_constant_is_37() {
    // draw_text.rs previously defined BLAME_COL_WIDTH=26 locally; it now
    // imports the shared constant from draw.rs which is 37. Assert the value
    // so any future accidental revert is caught immediately.
    assert_eq!(
        crate::ui::content::draw::BLAME_COL_WIDTH,
        37,
        "shared BLAME_COL_WIDTH must be 37 (not the old 26)"
    );
}

#[test]
fn render_inline_fallback_with_blame_does_not_panic() {
    let (mut app, _dir) = render_app();
    let screen = render(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.show_blame = true;
        app.content = vec!["line1".to_string(), "line2".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "line1".to_string())],
            vec![(Style::default(), "line2".to_string())],
        ];
    });
    assert!(!screen.is_empty());
}

/// Helper: renders `draw_content` into a buffer for pixel-level assertions.
fn render_buffer<F>(app: &mut crate::app::App, f: F) -> ratatui::buffer::Buffer
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

/// Read a row of the buffer as a string.
fn row_text(buf: &ratatui::buffer::Buffer, row: u16) -> String {
    let width = buf.area.width;
    let start = (row * width) as usize;
    let end = start + width as usize;
    buf.content()[start..end]
        .iter()
        .map(|c| c.symbol())
        .collect()
}

// ── word-wrap gutter/content alignment ─────────────────────────────────

#[test]
fn wrap_content_long_line_produces_correct_visual_rows() {
    // One long line wraps into 3 visual rows at width 10.
    // 23 chars -> 10 + 10 + 3 = 3 visual rows
    let content = vec![Line::from("aaaabbbbccccddddeeeefff")]; // 23 chars
    let gutter = vec![Line::from(Span::styled("1 ", Style::default()))];
    let (g, c, vmap, fold) = super::wrap_content(&content, &gutter, 10, 0, &[]);
    assert_eq!(g.len(), 3, "3 visual rows expected");
    assert_eq!(c.len(), 3, "3 visual content rows expected");
    assert_eq!(vmap, vec![0, 0, 0], "all visual rows map to logical line 0");
    // First gutter row keeps the number
    assert!(
        g[0].spans.iter().any(|s| s.content.as_ref() == "1 "),
        "gutter 0 shows line number"
    );
    // Continuation rows have blank gutter
    assert!(
        g[1].spans
            .iter()
            .all(|s| s.content.as_ref().chars().all(|c| c == ' ')),
        "gutter 1 is blank"
    );
    assert!(
        g[2].spans
            .iter()
            .all(|s| s.content.as_ref().chars().all(|c| c == ' ')),
        "gutter 2 is blank"
    );
    assert!(fold.is_empty(), "no fold rows expected");
}

#[test]
fn wrap_content_mixed_lines_maintains_alignment() {
    // Line 0 fits (no wrap), line 1 wraps into 2 rows, line 2 fits.
    let content = vec![
        Line::from("short"),         // fits in 10
        Line::from("this is long!"), // 14 chars -> 2 rows
        Line::from("tiny"),          // fits
    ];
    let gutters = vec![
        Line::from(Span::styled("1 ", Style::default())),
        Line::from(Span::styled("2 ", Style::default())),
        Line::from(Span::styled("3 ", Style::default())),
    ];
    let (g, c, vmap, _fold) = super::wrap_content(&content, &gutters, 10, 0, &[]);
    // Visual rows: line0 (1), line1 (2), line2 (1) = 4 total
    assert_eq!(g.len(), 4, "4 visual rows total");
    assert_eq!(c.len(), 4, "4 visual content rows total");
    assert_eq!(vmap, vec![0, 1, 1, 2], "visual->logical mapping");
    // Gutter: row 0 = "1 ", row 1 = "2 ", row 2 = blank, row 3 = "3 "
    assert_eq!(g[0].spans[0].content.as_ref(), "1 ");
    assert_eq!(g[1].spans[0].content.as_ref(), "2 ");
    assert!(
        g[2].spans[0].content.as_ref().chars().all(|c| c == ' '),
        "row 2 gutter blank"
    );
    assert_eq!(g[3].spans[0].content.as_ref(), "3 ");
}

#[test]
fn wrap_content_zero_width_returns_single_row() {
    let content = vec![Line::from("hi")];
    let gutters = vec![Line::from(Span::styled("1 ", Style::default()))];
    let (g, c, vmap, _fold) = super::wrap_content(&content, &gutters, 0, 0, &[]);
    assert_eq!(g.len(), 1);
    assert_eq!(c.len(), 1);
    assert_eq!(vmap, vec![0]);
}

#[test]
fn wrap_content_empty_lines_produce_one_visual_row_each() {
    let content = vec![Line::from(""), Line::from("")];
    let gutters = vec![
        Line::from(Span::styled("1 ", Style::default())),
        Line::from(Span::styled("2 ", Style::default())),
    ];
    let (g, c, vmap, _fold) = super::wrap_content(&content, &gutters, 10, 0, &[]);
    assert_eq!(g.len(), 2);
    assert_eq!(c.len(), 2);
    assert_eq!(vmap, vec![0, 1]);
    // Both gutter rows keep their numbers (no wrapping needed)
    assert_eq!(g[0].spans[0].content.as_ref(), "1 ");
    assert_eq!(g[1].spans[0].content.as_ref(), "2 ");
}

#[test]
fn wrap_content_does_not_wrap_within_fit() {
    // All lines fit: content unchanged.
    let content = vec![Line::from("abc"), Line::from("def")];
    let gutters = vec![
        Line::from(Span::styled("1 ", Style::default())),
        Line::from(Span::styled("2 ", Style::default())),
    ];
    let (g, c, vmap, _fold) = super::wrap_content(&content, &gutters, 10, 0, &[]);
    assert_eq!(g.len(), 2);
    assert_eq!(c.len(), 2);
    assert_eq!(c[0].spans[0].content.as_ref(), "abc");
    assert_eq!(c[1].spans[0].content.as_ref(), "def");
    assert_eq!(vmap, vec![0, 1]);
}

#[test]
fn wrap_content_fold_gutter_rows_adjusted() {
    let content = vec![
        Line::from("short"),         // 1 visual row
        Line::from("this is long!"), // 2 visual rows
    ];
    let gutters = vec![
        Line::from(Span::styled("1 ", Style::default())),
        Line::from(Span::styled("2 ", Style::default())),
    ];
    let fold_rows = vec![(5u16, 0usize), (6u16, 1usize)]; // gutter_y_base = 5
    let (_g, _c, _vmap, updated_fold) = super::wrap_content(&content, &gutters, 10, 5, &fold_rows);
    // Logical line 0 at offset 0 -> visual offset 0
    // Logical line 1 at offset 1 -> visual offset 1
    assert_eq!(updated_fold.len(), 2);
    assert_eq!(updated_fold[0], (5, 0), "first fold row stays at y=5");
    assert_eq!(
        updated_fold[1],
        (6, 1),
        "second fold row moves to y=6 (after line0's 1 row)"
    );
}

#[test]
fn wrap_content_fold_rows_shift_with_wrapped_line() {
    let content = vec![
        Line::from("this is long!"), // 2 visual rows
        Line::from("short"),         // 1 visual row
    ];
    let gutters = vec![
        Line::from(Span::styled("1 ", Style::default())),
        Line::from(Span::styled("2 ", Style::default())),
    ];
    let fold_rows = vec![(0u16, 0usize), (1u16, 1usize)]; // gutter_y_base = 0
    let (_g, _c, _vmap, updated_fold) = super::wrap_content(&content, &gutters, 10, 0, &fold_rows);
    // Logical line 0 (2 visual rows) -> visual offset 0, 1
    // Logical line 1 (1 visual row)  -> visual offset 2
    assert_eq!(updated_fold[0], (0, 0), "first fold row stays at y=0");
    assert_eq!(updated_fold[1], (2, 1), "second fold row shifts to y=2");
}

// ── End-to-end render tests for word-wrap gutter alignment ─────────────

#[test]
fn draw_word_wrap_gutter_shows_number_on_first_visual_row_only() {
    let (mut app, _dir) = render_app();
    let buf = render_buffer(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["x".repeat(160)]; // wraps into ~3 rows at 78 cols
        app.highlighted = vec![vec![(Style::default(), "x".repeat(160))]];
        app.show_line_numbers = true;
        app.word_wrap = true;
    });
    // Content rows start at y=1 (after top border). The first content row
    // has format: "│1 xxxxx..." where col 0 is the border, col 1 is the gutter.
    let r1 = row_text(&buf, 1);
    let gutter1: String = r1.chars().skip(1).take(4).collect();
    assert!(
        gutter1.contains('1'),
        "row 1 gutter should show line number 1, got gutter {gutter1:?} from row {r1:?}"
    );
    // Row 2 (continuation of the wrapped line) must have blank gutter.
    let r2 = row_text(&buf, 2);
    let gutter2: String = r2.chars().skip(1).take(4).collect();
    assert!(
        !gutter2.contains('1'),
        "continuation row gutter should be blank, got gutter: {gutter2:?} from row {r2:?}"
    );
    // Row 3 (continuation) must also have blank gutter.
    let r3 = row_text(&buf, 3);
    let gutter3: String = r3.chars().skip(1).take(4).collect();
    assert!(
        !gutter3.contains('1'),
        "second continuation row gutter should be blank, got: {gutter3:?} from row {r3:?}"
    );
}

#[test]
fn draw_word_wrap_mixed_lines_show_correct_gutter_numbers() {
    let (mut app, _dir) = render_app();
    let buf = render_buffer(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        // Line 0: short (fits); line 1: long (wraps to 2 rows); line 2: short
        app.content = vec!["first".to_string(), "x".repeat(140), "last".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "first".to_string())],
            vec![(Style::default(), "x".repeat(140))],
            vec![(Style::default(), "last".to_string())],
        ];
        app.show_line_numbers = true;
        app.word_wrap = true;
    });

    // Row for line 0 (y=1): gutter shows "1"
    let r1 = row_text(&buf, 1);
    let gutter1: String = r1.chars().skip(1).take(4).collect();
    assert!(
        gutter1.contains('1'),
        "line 0 must show gutter 1, got {gutter1:?}"
    );

    // Row for line 1 (y=2): gutter shows "2"
    let r2 = row_text(&buf, 2);
    let gutter2: String = r2.chars().skip(1).take(4).collect();
    assert!(
        gutter2.contains('2'),
        "line 1 must show gutter 2, got {gutter2:?}"
    );

    // Row 3 is continuation of line 1 -> gutter is blank (no "2")
    let r3 = row_text(&buf, 3);
    let gutter3: String = r3.chars().skip(1).take(4).collect();
    assert!(
        !gutter3.contains('2'),
        "continuation of wrapped line should have blank gutter: {gutter3:?}"
    );

    // The line after the wrapped one (last logical line) shows on its own visual row.
    let all: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(all.contains("last"), "content must contain 'last' line");
}

#[test]
fn draw_word_wrap_active_line_highlights_all_visual_rows() {
    let (mut app, _dir) = render_app();
    let active_bg = app.theme.active_line_bg;
    let buf = render_buffer(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["x".repeat(160)]; // wraps into ~3 rows
        app.highlighted = vec![vec![(Style::default(), "x".repeat(160))]];
        app.show_line_numbers = true;
        app.word_wrap = true;
        app.active_line = 0; // line 0 (the only line)
    });

    // The active-line highlight should appear on all 3 visual rows of the wrapped line.
    // Rows 1, 2, 3 (after border at row 0) should have active_line_bg somewhere.
    let has_bg_on_row = |y: u16| -> bool {
        buf.content().iter().enumerate().any(|(i, c)| {
            let row = (i as u16) / 80;
            row == y && c.bg == active_bg
        })
    };
    assert!(has_bg_on_row(1), "row 1 must have active-line bg");
    assert!(has_bg_on_row(2), "row 2 must have active-line bg (wrapped)");
    assert!(has_bg_on_row(3), "row 3 must have active-line bg (wrapped)");
}

#[test]
fn draw_word_wrap_off_behaves_unchanged() {
    let (mut app, _dir) = render_app();
    let buf = render_buffer(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.content = vec!["hello".to_string(), "x".repeat(160), "world".to_string()];
        app.highlighted = vec![
            vec![(Style::default(), "hello".to_string())],
            vec![(Style::default(), "x".repeat(160))],
            vec![(Style::default(), "world".to_string())],
        ];
        app.show_line_numbers = true;
        app.word_wrap = false; // OFF — each line is one gutter row, long lines overflow
    });

    // Without wrap, each logical line produces exactly one visual row.
    // Gutter should show 1, 2, 3 on consecutive rows (border at col 0, gutter at col 1).
    let r1 = row_text(&buf, 1);
    let gutter1: String = r1.chars().skip(1).take(4).collect();
    assert!(
        gutter1.contains('1'),
        "row 1 gutter shows 1, got {gutter1:?}"
    );
    let r2 = row_text(&buf, 2);
    let gutter2: String = r2.chars().skip(1).take(4).collect();
    assert!(
        gutter2.contains('2'),
        "row 2 gutter shows 2, got {gutter2:?}"
    );
    let r3 = row_text(&buf, 3);
    let gutter3: String = r3.chars().skip(1).take(4).collect();
    assert!(
        gutter3.contains('3'),
        "row 3 gutter shows 3, got {gutter3:?}"
    );
    // The long line (line 2) overflows rather than wrapping, so no extra rows.
    let all: String = buf.content().iter().map(|c| c.symbol()).collect();
    assert!(all.contains("hello"), "hello should render");
    assert!(all.contains("world"), "world should render");
}
