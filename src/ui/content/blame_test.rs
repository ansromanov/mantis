//! Tests for blame-annotation rendering in the content pane.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

use crate::app::App;
use crate::config::Config;

use super::*;

#[test]
fn blame_strip_width_respects_bounds() {
    // At least 25, at most 40, roughly 30%
    assert_eq!(blame_strip_width(100), 30);
    assert_eq!(blame_strip_width(30), 20); // 25 min overridden: only 10 left for content
    assert_eq!(blame_strip_width(200), 40); // max
    assert_eq!(blame_strip_width(80), 25); // 30% = 24, clamped to min 25
    assert_eq!(blame_strip_width(10), 0); // can't leave 10 for content
}

#[test]
fn draw_blame_annotations_empty_blame_lines_does_not_panic() {
    let mut app = App::new(std::path::PathBuf::from("."), Config::default(), None, None).unwrap();
    app.content = (0..10).map(|i| format!("line {i}")).collect();
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_blame_annotations(frame, &mut app, frame.area(), &[]);
        })
        .unwrap();
}

#[test]
fn draw_bottom_bar_blame_without_current_file_does_not_panic() {
    let mut app = App::new(std::path::PathBuf::from("."), Config::default(), None, None).unwrap();
    app.current_file = None;
    let backend = TestBackend::new(40, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            draw_bottom_bar_blame(
                frame,
                &mut app,
                Rect {
                    x: 0,
                    y: 18,
                    width: 40,
                    height: 2,
                },
            );
        })
        .unwrap();
}
