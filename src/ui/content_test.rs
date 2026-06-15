use super::*;
use crate::search::InFileSearch;
use crate::theme::Theme;

fn default_theme() -> Theme {
    Theme::default()
}

fn single_region(text: &str) -> Vec<(Style, String)> {
    vec![(Style::default(), text.to_string())]
}

fn multi_region(parts: &[&str]) -> Vec<(Style, String)> {
    parts
        .iter()
        .map(|t| (Style::default(), t.to_string()))
        .collect()
}

// ── apply_selection ───────────────────────────────────────────────────────

#[test]
fn selection_empty_cols_returns_unmodified() {
    let regions = single_region("hello world");
    let result = apply_selection(&regions, 0, 0, Color::Red);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello world");
}

#[test]
fn selection_highlights_middle_range() {
    let regions = single_region("hello world");
    let result = apply_selection(&regions, 6, 11, Color::Red);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello ");
    assert_eq!(result[1].content, "world");
    assert_eq!(result[1].style.bg, Some(Color::Red));
}

#[test]
fn selection_highlights_start_of_region() {
    let regions = single_region("hello");
    let result = apply_selection(&regions, 0, 3, Color::Blue);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hel");
    assert_eq!(result[0].style.bg, Some(Color::Blue));
    assert_eq!(result[1].content, "lo");
}

#[test]
fn selection_col_end_usize_max_goes_to_end() {
    let regions = single_region("test");
    let result = apply_selection(&regions, 2, usize::MAX, Color::Green);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "te");
    assert_eq!(result[1].content, "st");
    assert_eq!(result[1].style.bg, Some(Color::Green));
}

#[test]
fn selection_spans_multiple_regions() {
    let regions = multi_region(&["abc", "def", "ghi"]);
    let result = apply_selection(&regions, 2, 7, Color::Yellow);
    let total: String = result.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(total, "abcdefghi");
    let selected: Vec<&Span> = result
        .iter()
        .filter(|s| s.style.bg == Some(Color::Yellow))
        .collect();
    let selected_text: String = selected.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(selected_text, "cdefg");
}

#[test]
fn selection_covers_entire_text() {
    let regions = single_region("full");
    let result = apply_selection(&regions, 0, 4, Color::Magenta);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "full");
    assert_eq!(result[0].style.bg, Some(Color::Magenta));
}

#[test]
fn selection_col_start_past_end() {
    let regions = single_region("hi");
    let result = apply_selection(&regions, 10, 20, Color::Red);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hi");
    assert_eq!(result[0].style.bg, None);
}

// ── apply_search_to_regions ───────────────────────────────────────────────

fn make_search(matches: Vec<crate::search::InFileMatch>, current: usize) -> InFileSearch {
    InFileSearch {
        query: "test".to_string(),
        matches,
        current,
    }
}

#[test]
fn search_no_matches_returns_unmodified() {
    let regions = single_region("hello world");
    let search = InFileSearch::new();
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello world");
}

#[test]
fn search_highlights_current_match() {
    let regions = single_region("abcde");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 0,
            col: 1,
            len: 3,
        }],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].content, "a");
    assert_eq!(result[1].content, "bcd");
    assert_eq!(result[1].style.bg, Some(default_theme().selection_bg));
    assert_eq!(result[2].content, "e");
}

#[test]
fn search_non_current_match_uses_dim_bg() {
    let regions = single_region("abcde");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 0,
            col: 1,
            len: 3,
        }],
        1,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result[1].style.bg, Some(default_theme().dim));
}

#[test]
fn search_multiple_matches_on_line() {
    let regions = single_region("aa bb aa");
    let search = make_search(
        vec![
            crate::search::InFileMatch {
                line: 0,
                col: 0,
                len: 2,
            },
            crate::search::InFileMatch {
                line: 0,
                col: 6,
                len: 2,
            },
        ],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    let highlighted: String = result
        .iter()
        .filter(|s| s.style.bg == Some(default_theme().selection_bg))
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(highlighted, "aa");
}

#[test]
fn search_skips_other_lines() {
    let regions = single_region("hello");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 1,
            col: 0,
            len: 3,
        }],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello");
}

#[test]
fn search_match_at_start_of_region() {
    let regions = single_region("hello");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 0,
            col: 0,
            len: 2,
        }],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "he");
    assert_eq!(result[0].style.bg, Some(default_theme().selection_bg));
    assert_eq!(result[1].content, "llo");
}

#[test]
fn search_match_at_end_of_region() {
    let regions = single_region("hello");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 0,
            col: 3,
            len: 2,
        }],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hel");
    assert_eq!(result[1].content, "lo");
    assert_eq!(result[1].style.bg, Some(default_theme().selection_bg));
}

#[test]
fn search_multi_byte_chars() {
    let regions = single_region("héllo wörld");
    let search = make_search(
        vec![crate::search::InFileMatch {
            line: 0,
            col: 4,
            len: 2,
        }],
        0,
    );
    let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
    let total: String = result.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(total, "héllo wörld");
}

// ── emphasize ──────────────────────────────────────────────────────────

#[test]
fn emphasize_no_ranges_returns_full_text() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("hello", &[], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello");
}

#[test]
fn emphasize_middle_range_splits_correctly() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("hello world", &[(6, 11)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello ");
    assert_eq!(result[0].style, base);
    assert_eq!(result[1].content, "world");
    assert_eq!(result[1].style, emph);
}

#[test]
fn emphasize_range_at_start() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Blue);
    let result = emphasize("abcdef", &[(0, 3)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "abc");
    assert_eq!(result[0].style, emph);
    assert_eq!(result[1].content, "def");
    assert_eq!(result[1].style, base);
}

#[test]
fn emphasize_range_at_end() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Green);
    let result = emphasize("abcdef", &[(3, 6)], base, emph);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "abc");
    assert_eq!(result[0].style, base);
    assert_eq!(result[1].content, "def");
    assert_eq!(result[1].style, emph);
}

#[test]
fn emphasize_full_range() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Yellow);
    let result = emphasize("full", &[(0, 4)], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "full");
    assert_eq!(result[0].style, emph);
}

#[test]
fn emphasize_multiple_disjoint_ranges() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Magenta);
    let result = emphasize("abcdefghi", &[(1, 3), (5, 8)], base, emph);
    let joined: String = result.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(joined, "abcdefghi");
    let emphasized: String = result
        .iter()
        .filter(|s| s.style.bg == Some(Color::Magenta))
        .map(|s| s.content.as_ref())
        .collect();
    assert_eq!(emphasized, "bcfgh");
}

#[test]
fn emphasize_empty_text() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Red);
    let result = emphasize("", &[(0, 0)], base, emph);
    assert_eq!(result.len(), 0);
}

#[test]
fn emphasize_range_out_of_bounds_clamps() {
    let base = Style::default();
    let emph = Style::default().bg(Color::Cyan);
    let result = emphasize("hi", &[(10, 20)], base, emph);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hi");
    assert_eq!(result[0].style, base);
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
fn draw_markdown_mode() {
    let (mut app, _dir) = render_app();
    let _buffer = render_content(&mut app, |app| {
        app.current_file = None;
        app.virtual_file = None;
        app.is_markdown = true;
        app.show_raw_markdown = false;
        app.markdown_lines = vec![
            vec![(Style::default().fg(Color::Cyan), "# Title".to_string())],
            vec![(Style::default(), "body text".to_string())],
        ];
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
