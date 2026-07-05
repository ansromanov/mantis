use ratatui::style::Style;

use crate::search::InFileSearch;
use crate::theme::Theme;
use crate::ui::content::search::apply_search_to_regions;

fn default_theme() -> Theme {
    Theme::default()
}

fn single_region(text: &str) -> Vec<(Style, String)> {
    vec![(Style::default(), text.to_string())]
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

#[test]
fn search_current_vs_other_distinct_bgs_on_one_line() {
    // Two matches on the same logical line; only the `current` one gets
    // selection_bg, the other gets dim. Order in the span vec follows column
    // order regardless of which is current.
    let regions = single_region("aa bb aa");
    let theme = default_theme();
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
        1, // second match is current
    );
    let result = apply_search_to_regions(&regions, 0, &search, &theme);
    let current: String = result
        .iter()
        .filter(|s| s.style.bg == Some(theme.selection_bg))
        .map(|s| s.content.as_ref())
        .collect();
    let other: String = result
        .iter()
        .filter(|s| s.style.bg == Some(theme.dim))
        .map(|s| s.content.as_ref())
        .collect();
    // Both occurrences are the literal "aa"; the current is the trailing one.
    assert_eq!(current, "aa");
    assert_eq!(other, "aa");
    // Trailing "aa" (current) must come after the leading "aa" (other) in order.
    let cur_idx = result
        .iter()
        .position(|s| s.style.bg == Some(theme.selection_bg))
        .unwrap();
    let oth_idx = result
        .iter()
        .position(|s| s.style.bg == Some(theme.dim))
        .unwrap();
    assert!(
        oth_idx < cur_idx,
        "leading match should render before trailing"
    );
}
