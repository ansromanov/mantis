use super::*;

use ratatui::layout::Rect;

#[test]
fn centered_rect_is_centered() {
    let area = Rect::new(0, 0, 100, 100);
    let r = centered_rect(50, 50, area);
    assert_eq!(r.width, 50);
    assert_eq!(r.height, 50);
    assert_eq!(r.x, 25);
    assert_eq!(r.y, 25);
}

#[test]
fn search_toggle_spans_marks_active_toggles() {
    let theme = Theme::default();
    let spans = search_toggle_spans(true, false, false, &theme);
    assert_eq!(spans.len(), 5);
    assert_eq!(spans[0].content, "[Aa]");
    assert_eq!(spans[2].content, r"[\b]");
    assert_eq!(spans[4].content, "[.*]");
    // Active toggle is bold, inactive ones are not.
    assert!(spans[0].style.add_modifier.contains(Modifier::BOLD));
    assert!(!spans[2].style.add_modifier.contains(Modifier::BOLD));
    assert!(!spans[4].style.add_modifier.contains(Modifier::BOLD));
}
