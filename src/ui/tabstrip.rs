//! The tab strip: one row listing every open project tab.
//!
//! Only drawn (by `ui::draw_workspace`) when more than one tab is open, or
//! while the "open project as new tab" prompt is active — a single-tab
//! session looks exactly like it did before tabs existed. Each tab shows the
//! project root's directory name; the active tab is highlighted and every
//! tab has a `×` close glyph. When tabs exceed the available width, the strip
//! shows `‹`/`›` affordances and scrolls horizontally. Column ranges are
//! recomputed identically by [`draw_tabstrip`] and [`hit_test`] so mouse clicks
//! land on the same tab or affordance the strip visually shows.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::workspace::Tabs;

/// What a mouse click on the tab strip landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabHit {
    Switch(usize),
    Close(usize),
    ScrollLeft,
    ScrollRight,
}

pub(crate) const MAX_LABEL_LEN: usize = 20;
pub(crate) const MIN_LABEL_LEN: usize = 7;
pub(crate) const AFFORDANCE_WIDTH: u16 = 2;

/// The display label for a tab: its root directory's file name, truncated.
fn tab_label_with_max(app: &crate::app::App, max_len: usize) -> String {
    let name = app
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.root.display().to_string());
    if name.chars().count() > max_len {
        let truncated: String = name.chars().take(max_len.saturating_sub(1)).collect();
        format!("{truncated}…")
    } else {
        name
    }
}

#[cfg(test)]
fn tab_label(app: &crate::app::App) -> String {
    tab_label_with_max(app, MAX_LABEL_LEN)
}

pub(crate) struct Segment {
    pub(crate) start: u16,
    pub(crate) end: u16,
    pub(crate) hit: TabHit,
    pub(crate) label: String,
    pub(crate) badge: String,
}

fn tab_width(app: &crate::app::App, max_len: usize) -> u16 {
    tab_label_with_max(app, max_len).chars().count() as u16 + 4
}

fn compute_max_label_len(tabs: &Tabs, width: u16) -> usize {
    for max_len in (MIN_LABEL_LEN..=MAX_LABEL_LEN).rev() {
        let total: u16 = tabs.apps.iter().map(|app| tab_width(app, max_len)).sum();
        if total <= width {
            return max_len;
        }
    }
    MIN_LABEL_LEN
}

fn tab_segment(tabs: &Tabs, index: usize, max_len: usize, remaining: u16) -> (u16, String, String) {
    let Some(app) = tabs.apps.get(index) else {
        return (0, String::new(), String::new());
    };
    let label = unique_tab_label_with_max(tabs, index, max_len);
    let badge = tab_badge(app);
    let badge = if label.chars().count() + badge.chars().count() + 4 <= remaining as usize {
        badge
    } else {
        String::new()
    };
    let width = label.chars().count() as u16 + badge.chars().count() as u16 + 4;
    (width, label, badge)
}

/// Builds the column ranges for each tab's switch region and close glyph,
/// stopping once the strip runs out of horizontal room.
pub(crate) fn build_segments_with(tabs: &Tabs, first_visible: usize, area: Rect) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut x = area.x;
    let right = area.x.saturating_add(area.width);
    if tabs.apps.is_empty() || area.width == 0 {
        return segments;
    }
    let max_label_len = compute_max_label_len(tabs, area.width);
    let first_visible = first_visible.min(tabs.apps.len().saturating_sub(1));
    if first_visible > 0 {
        let end = x.saturating_add(AFFORDANCE_WIDTH).min(right);
        if end > x {
            segments.push(Segment {
                start: x,
                end,
                hit: TabHit::ScrollLeft,
                label: String::new(),
                badge: String::new(),
            });
            x = end;
        }
    }
    let mut last_rendered = None;
    for i in first_visible..tabs.apps.len() {
        let (width, label, badge) = tab_segment(tabs, i, max_label_len, right.saturating_sub(x));
        let has_more = i + 1 < tabs.apps.len();
        let reserve_right = has_more;
        if x.saturating_add(width)
            .saturating_add(if reserve_right { AFFORDANCE_WIDTH } else { 0 })
            > right
        {
            break;
        }
        let close_start = x + width - 2;
        segments.push(Segment {
            start: x,
            end: close_start,
            hit: TabHit::Switch(i),
            label: label.clone(),
            badge: badge.clone(),
        });
        segments.push(Segment {
            start: close_start,
            end: x + width,
            hit: TabHit::Close(i),
            label,
            badge,
        });
        x += width;
        last_rendered = Some(i);
    }
    let has_unrendered = last_rendered.is_none_or(|last| last + 1 < tabs.apps.len());
    if has_unrendered && x < right {
        let end = x.saturating_add(AFFORDANCE_WIDTH).min(right);
        if end > x {
            segments.push(Segment {
                start: x,
                end,
                hit: TabHit::ScrollRight,
                label: String::new(),
                badge: String::new(),
            });
        }
    }
    segments
}

fn build_segments(tabs: &Tabs, area: Rect) -> Vec<Segment> {
    build_segments_with(tabs, tabs.first_visible, area)
}

/// Returns whether `target` is rendered when the strip starts at `first_visible`.
pub(crate) fn is_tab_visible(tabs: &Tabs, first_visible: usize, target: usize, width: u16) -> bool {
    build_segments_with(tabs, first_visible, Rect::new(0, 0, width, 1))
        .iter()
        .any(|segment| segment.hit == TabHit::Switch(target))
}

pub(crate) fn draw_tabstrip(f: &mut Frame, tabs: &mut Tabs, area: Rect) {
    tabs.strip_area = area;
    tabs.first_visible = tabs.first_visible.min(tabs.apps.len().saturating_sub(1));
    let theme = &tabs.active_app().theme;
    let base = Style::default().bg(theme.dim);
    f.render_widget(Paragraph::new(Line::from("")).style(base), area);

    if let Some(query) = &tabs.new_tab_prompt {
        let text = format!(" open project: {query}\u{2588}");
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                text,
                Style::default().fg(theme.accent_alt),
            )))
            .style(base),
            area,
        );
        return;
    }

    let segments = build_segments(tabs, area);
    let mut spans = Vec::new();
    if segments
        .iter()
        .any(|segment| segment.hit == TabHit::ScrollLeft)
    {
        spans.push(Span::styled(
            "‹ ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ));
    }
    for segment in &segments {
        let TabHit::Switch(i) = segment.hit else {
            continue;
        };
        let active = i == tabs.active;
        let style = if active {
            theme.selection_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        spans.push(Span::styled(format!(" {}", segment.label), style));
        if !segment.badge.is_empty() {
            spans.push(Span::styled(
                segment.badge.as_str(),
                Style::default().fg(theme.accent_alt),
            ));
        }
        spans.push(Span::styled(" ", style));
        spans.push(Span::styled("×", Style::default().fg(theme.dim)));
        spans.push(Span::raw(" "));
    }
    if segments
        .iter()
        .any(|segment| segment.hit == TabHit::ScrollRight)
    {
        spans.push(Span::styled(
            " ›",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
}

/// Produces the shortest trailing path suffix that uniquely identifies a tab.
#[cfg(test)]
fn unique_tab_label(tabs: &Tabs, index: usize) -> String {
    unique_tab_label_with_max(tabs, index, MAX_LABEL_LEN)
}

fn unique_tab_label_with_max(tabs: &Tabs, index: usize, max_len: usize) -> String {
    let Some(app) = tabs.apps.get(index) else {
        return String::new();
    };
    let parts = app
        .root
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if let Some(name) = parts.last() {
        let collision = tabs.apps.iter().enumerate().any(|(other_index, other)| {
            other_index != index
                && other
                    .root
                    .file_name()
                    .is_some_and(|other_name| other_name.to_string_lossy() == *name)
        });
        if !collision {
            return tab_label_with_max(app, max_len);
        }
    }
    let mut count = 1;
    while count < parts.len() {
        let candidate = parts[parts.len() - count..].join("/");
        let unique = tabs.apps.iter().enumerate().all(|(other_index, other)| {
            other_index == index
                || other
                    .root
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .take(count)
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("/")
                    != candidate
        });
        if unique {
            break;
        }
        count += 1;
    }
    let label = parts[parts.len().saturating_sub(count)..].join("/");
    if label.chars().count() > max_len {
        let truncated: String = label.chars().take(max_len.saturating_sub(1)).collect();
        format!("{truncated}…")
    } else {
        label
    }
}

fn tab_badge(app: &crate::app::App) -> String {
    let Some(info) = &app.git_info else {
        return String::new();
    };
    let mut badge = format!("·{}", info.head.display());
    if info.total_changed > 0 {
        badge.push_str(&format!(" ●{}", info.total_changed));
    }
    badge
}

/// Maps a mouse click at `(col, row)` to the tab it landed on, or `None`
/// when the click missed the strip or landed past the last tab that fit.
pub(crate) fn hit_test(tabs: &Tabs, area: Rect, col: u16, row: u16) -> Option<TabHit> {
    if tabs.new_tab_prompt.is_some()
        || row != area.y
        || col < area.x
        || col >= area.x.saturating_add(area.width)
    {
        return None;
    }
    build_segments(tabs, area)
        .into_iter()
        .find(|s| col >= s.start && col < s.end)
        .map(|s| s.hit)
}

#[cfg(test)]
#[path = "tabstrip_test.rs"]
mod tests;
