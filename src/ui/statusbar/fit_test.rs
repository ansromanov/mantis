use super::*;

use ratatui::style::{Color, Style};

/// A deterministic theme for pure-function tests (separator/color/elision
/// math): every role resolves, and `accent` is pinned to red.
fn test_theme() -> Theme {
    let mut t = Theme::monochrome();
    t.accent = Color::Red;
    t.dim = Color::Gray;
    t.text = Color::White;
    t
}

fn seg(s: &str, kind: StatusSegment, prio: u8) -> Seg {
    (Span::styled(s.to_string(), Style::default()), kind, prio)
}

#[test]
fn fit_segments_empty_input() {
    let cfg = StatusBarConfig::default();
    let line = fit_two_sided(vec![], 80, &cfg, &test_theme());
    assert_eq!(line.width(), 0);
}

#[test]
fn fit_segments_zero_max_width() {
    let cfg = StatusBarConfig::default();
    let segs = vec![
        seg("hello", StatusSegment::Badges, P_INFO),
        seg("world", StatusSegment::Lnum, P_VER),
    ];
    let line = fit_two_sided(segs, 0, &cfg, &test_theme());
    assert_eq!(line.width(), 0);
}

#[test]
fn both_none_default_split() {
    // Regression: both None should behave like the old default.
    let cfg = StatusBarConfig::default();
    let segs = vec![
        seg("badges", StatusSegment::Badges, P_INFO),
        seg("ver", StatusSegment::Version, P_VER),
        seg("git", StatusSegment::Git, P_GIT),
    ];
    let (left, right) = split_sides(segs, &cfg);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].0.content.as_ref(), "badges");
    assert_eq!(right.len(), 2);
    assert_eq!(right[0].0.content.as_ref(), "ver");
    assert_eq!(right[1].0.content.as_ref(), "git");
}

#[test]
fn split_sides_default_mode_natural_order() {
    // Default mode: both None partition by `side()`, preserving build order
    // within each group.
    let cfg = StatusBarConfig::default();
    let segs = vec![
        seg("badges", StatusSegment::Badges, P_INFO),
        seg("ver", StatusSegment::Version, P_VER),
        seg("git", StatusSegment::Git, P_GIT),
        seg("scroll", StatusSegment::Scroll, P_INFO),
    ];
    let (left, right) = split_sides(segs, &cfg);
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].0.content.as_ref(), "badges");
    assert_eq!(left[1].0.content.as_ref(), "scroll");
    assert_eq!(right.len(), 2);
    assert_eq!(right[0].0.content.as_ref(), "ver");
    assert_eq!(right[1].0.content.as_ref(), "git");
}

#[test]
fn split_sides_explicit_mode_order_follows_config() {
    let cfg = StatusBarConfig {
        left: Some(vec!["git".into(), "badges".into()]),
        right: Some(vec!["version".into()]),
        ..Default::default()
    };
    let segs = vec![
        seg("badges", StatusSegment::Badges, P_INFO),
        seg("ver", StatusSegment::Version, P_VER),
        seg("git", StatusSegment::Git, P_GIT),
    ];
    let (left, right) = split_sides(segs, &cfg);
    // Left order follows config: git before badges.
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].0.content.as_ref(), "git");
    assert_eq!(left[1].0.content.as_ref(), "badges");
    // Right: version.
    assert_eq!(right.len(), 1);
    assert_eq!(right[0].0.content.as_ref(), "ver");
}

#[test]
fn elide_drops_lowest_priority_first() {
    let segs = vec![
        seg("meta", StatusSegment::Message, P_META),
        seg("info", StatusSegment::Badges, P_INFO),
        seg("ver", StatusSegment::Version, P_VER),
    ];
    // With the default separator each span costs width+1: meta 5, info 5,
    // ver 4, total 14. At max 9, 'meta' (lowest priority) is dropped first
    // and 'info'+'ver' then fit.
    let kept = elide(segs, 9, " ");
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].0.content.as_ref(), "info");
    assert_eq!(kept[1].0.content.as_ref(), "ver");
}

#[test]
fn elide_keeps_everything_when_it_fits() {
    let segs = vec![
        seg("aa", StatusSegment::Badges, P_INFO),
        seg("bb", StatusSegment::Folds, P_INFO),
    ];
    let kept = elide(segs, 10, " ");
    assert_eq!(kept.len(), 2);
}

#[test]
fn compose_left_right_padding() {
    let left = vec![Span::styled("left", Style::default())];
    let right = vec![Span::styled("right", Style::default())];
    // Separator (1) + left (4) = 5; right group = separator (1) + right (5) =
    // 6; gap = 20 - 11 = 9, immediately followed by the right group's leading
    // separator space → 10 spaces between "left" and "right".
    let line = compose_left_right(left, right, 20, " ");
    assert_eq!(line.width(), 20);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, " left          right");
}

#[test]
fn compose_left_right_no_padding_needed() {
    let left = vec![Span::styled("abc", Style::default())];
    let right = vec![Span::styled("de", Style::default())];
    // Separator+abc = 4, separator+de = 3, total 7 = max → no gap, but each
    // group still leads with its separator.
    let line = compose_left_right(left, right, 7, " ");
    assert_eq!(line.width(), 7);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, " abc de");
}

#[test]
fn compose_left_right_custom_separator() {
    let left = vec![Span::styled("abc", Style::default())];
    let right = vec![Span::styled("de", Style::default())];
    // Custom 3-char separator before every segment. Left group = 3+3=6,
    // right group = 3+2=5, gap = 20-11 = 9, then the right group's leading
    // separator space → 10 spaces before the box char.
    let line = compose_left_right(left, right, 20, " \u{2502} ");
    assert_eq!(line.width(), 20);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, " \u{2502} abc          \u{2502} de");
}

#[test]
fn compose_left_right_exact_fit() {
    let left = vec![Span::styled("abc", Style::default())];
    let right = vec![Span::styled("de", Style::default())];
    // Left = sep+abc = 4, right = sep+de = 3, total 7 = max → no gap, but the
    // group-leading separators still show.
    let line = compose_left_right(left, right, 7, " ");
    assert_eq!(line.width(), 7);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, " abc de");
}

#[test]
fn compose_left_right_empty_groups() {
    let line = compose_left_right(vec![], vec![], 10, " ");
    assert_eq!(line.width(), 10);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "          ");
}

#[test]
fn wider_separator_increases_elision_budget() {
    // A wider separator trades against segment width in the elision budget:
    // each span here has no baked-in leading space, so the rendered total is
    // sep(3)+1 per segment = 8 for two, 4 for one. At max 8 both survive; at
    // max 7 the lower-priority one is dropped.
    let cfg = StatusBarConfig {
        separator: " \u{2502} ".into(),
        ..Default::default()
    };
    let segs = vec![
        seg("a", StatusSegment::Badges, P_INFO),
        seg("b", StatusSegment::Folds, P_INFO),
    ];
    let line = fit_two_sided(segs.clone(), 8, &cfg, &test_theme());
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, " \u{2502} a \u{2502} b");
    let line = fit_two_sided(segs, 7, &cfg, &test_theme());
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    // With no right group, leftover width becomes trailing padding.
    assert_eq!(content.trim_end(), " \u{2502} a");
}

fn green_span(s: &str) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(Color::Green))
}

#[test]
fn color_override_recolors_segment_fg() {
    let cfg = StatusBarConfig {
        colors: [("badges".to_string(), "accent".to_string())]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let segs = vec![(green_span("badges"), StatusSegment::Badges, P_INFO)];
    let line = fit_two_sided(segs, 80, &cfg, &test_theme());
    let span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "badges")
        .expect("badges span should render");
    assert_eq!(span.style.fg, Some(Color::Red));
}

#[test]
fn color_override_ignores_unknown_role() {
    let cfg = StatusBarConfig {
        colors: [("badges".to_string(), "nonexistent_role".to_string())]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let segs = vec![(green_span("badges"), StatusSegment::Badges, P_INFO)];
    let line = fit_two_sided(segs, 80, &cfg, &test_theme());
    let span = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "badges")
        .expect("badges span should render");
    assert_eq!(span.style.fg, Some(Color::Green));
}

#[test]
fn color_override_applies_only_to_listed_segment() {
    let cfg = StatusBarConfig {
        colors: [("folds".to_string(), "accent".to_string())]
            .into_iter()
            .collect(),
        ..Default::default()
    };
    let segs = vec![
        (green_span("badges"), StatusSegment::Badges, P_INFO),
        (green_span("folds"), StatusSegment::Folds, P_INFO),
    ];
    let line = fit_two_sided(segs, 80, &cfg, &test_theme());
    let badges = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "badges")
        .expect("badges should render");
    let folds = line
        .spans
        .iter()
        .find(|s| s.content.as_ref() == "folds")
        .expect("folds should render");
    assert_eq!(badges.style.fg, Some(Color::Green));
    assert_eq!(folds.style.fg, Some(Color::Red));
}
