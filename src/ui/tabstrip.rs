//! The tab strip: one row listing every open project tab.
//!
//! Only drawn (by `ui::draw_workspace`) when more than one tab is open, or
//! while the "open project as new tab" prompt is active — a single-tab
//! session looks exactly like it did before tabs existed. Each tab shows the
//! project root's directory name; the active tab is highlighted and every
//! tab has a `×` close glyph. Column ranges are recomputed identically by
//! [`draw_tabstrip`] and [`hit_test`] so mouse clicks land on the same tab
//! the strip visually shows.

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
}

const MAX_LABEL_LEN: usize = 20;

/// The display label for a tab: its root directory's file name, truncated.
fn tab_label(app: &crate::app::App) -> String {
    let name = app
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.root.display().to_string());
    if name.chars().count() > MAX_LABEL_LEN {
        let truncated: String = name.chars().take(MAX_LABEL_LEN - 1).collect();
        format!("{truncated}…")
    } else {
        name
    }
}

struct Segment {
    start: u16,
    end: u16,
    hit: TabHit,
    label: String,
    badge: String,
}

/// Builds the column ranges for each tab's switch region and close glyph,
/// stopping once the strip runs out of horizontal room.
fn build_segments(tabs: &Tabs, area: Rect) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut x = area.x;
    let right = area.x.saturating_add(area.width);
    for (i, app) in tabs.apps.iter().enumerate() {
        let label = unique_tab_label(tabs, i);
        let badge = tab_badge(app);
        let badge = if label.chars().count() + badge.chars().count() + 4
            <= right.saturating_sub(x) as usize
        {
            badge
        } else {
            String::new()
        };
        // " label × " — one leading space, the label, " × " for the close glyph.
        let width = label.chars().count() as u16 + badge.chars().count() as u16 + 4;
        if x.saturating_add(width) > right {
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
    }
    segments
}

pub(crate) fn draw_tabstrip(f: &mut Frame, tabs: &mut Tabs, area: Rect) {
    tabs.strip_area = area;
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
    for i in 0..tabs.apps.len() {
        // Each tab contributes exactly two `build_segments` entries (switch,
        // close); stop rendering once we run past what fit.
        if segments.len() < i * 2 + 2 {
            break;
        }
        let Some(segment) = segments.get(i * 2) else {
            break;
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
    f.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
}

/// Produces the shortest trailing path suffix that uniquely identifies a tab.
fn unique_tab_label(tabs: &Tabs, index: usize) -> String {
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
            return tab_label(app);
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
    if label.chars().count() > MAX_LABEL_LEN {
        let truncated: String = label.chars().take(MAX_LABEL_LEN - 1).collect();
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
    if row != area.y || col < area.x || col >= area.x.saturating_add(area.width) {
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
