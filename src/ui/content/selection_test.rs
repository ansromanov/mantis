use ratatui::style::{Color, Style};
use ratatui::text::Span;

use crate::ui::content::selection::apply_selection;

fn single_region(text: &str) -> Vec<(Style, String)> {
    vec![(Style::default(), text.to_string())]
}

fn multi_region(parts: &[&str]) -> Vec<(Style, String)> {
    parts
        .iter()
        .map(|t| (Style::default(), t.to_string()))
        .collect()
}

#[test]
fn selection_empty_cols_returns_unmodified() {
    let regions = single_region("hello world");
    let result = apply_selection(&regions, 0, 0, Color::Red, false);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hello world");
}

#[test]
fn selection_highlights_middle_range() {
    let regions = single_region("hello world");
    let result = apply_selection(&regions, 6, 11, Color::Red, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello ");
    assert_eq!(result[1].content, "world");
    assert_eq!(result[1].style.bg, Some(Color::Red));
}

#[test]
fn selection_highlights_start_of_region() {
    let regions = single_region("hello");
    let result = apply_selection(&regions, 0, 3, Color::Blue, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hel");
    assert_eq!(result[0].style.bg, Some(Color::Blue));
    assert_eq!(result[1].content, "lo");
}

#[test]
fn selection_col_end_usize_max_goes_to_end() {
    let regions = single_region("test");
    let result = apply_selection(&regions, 2, usize::MAX, Color::Green, false);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "te");
    assert_eq!(result[1].content, "st");
    assert_eq!(result[1].style.bg, Some(Color::Green));
}

#[test]
fn selection_spans_multiple_regions() {
    let regions = multi_region(&["abc", "def", "ghi"]);
    let result = apply_selection(&regions, 2, 7, Color::Yellow, false);
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
    let result = apply_selection(&regions, 0, 4, Color::Magenta, false);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "full");
    assert_eq!(result[0].style.bg, Some(Color::Magenta));
}

#[test]
fn selection_col_start_past_end() {
    let regions = single_region("hi");
    let result = apply_selection(&regions, 10, 20, Color::Red, false);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].content, "hi");
    assert_eq!(result[0].style.bg, None);
}

#[test]
fn selection_monochrome_reverses() {
    let regions = single_region("hello world");
    let result = apply_selection(&regions, 6, 11, Color::Reset, true);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].content, "hello ");
    assert_eq!(result[1].content, "world");
    assert!(result[1]
        .style
        .add_modifier
        .contains(ratatui::style::Modifier::REVERSED));
}
