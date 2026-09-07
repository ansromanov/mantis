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
}

/// Builds the column ranges for each tab's switch region and close glyph,
/// stopping once the strip runs out of horizontal room.
fn build_segments(tabs: &Tabs, area: Rect) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut x = area.x;
    let right = area.x.saturating_add(area.width);
    for (i, app) in tabs.apps.iter().enumerate() {
        let label = tab_label(app);
        // " label × " — one leading space, the label, " × " for the close glyph.
        let width = label.chars().count() as u16 + 4;
        if x.saturating_add(width) > right {
            break;
        }
        let close_start = x + width - 2;
        segments.push(Segment {
            start: x,
            end: close_start,
            hit: TabHit::Switch(i),
        });
        segments.push(Segment {
            start: close_start,
            end: x + width,
            hit: TabHit::Close(i),
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
    for (i, app) in tabs.apps.iter().enumerate() {
        // Each tab contributes exactly two `build_segments` entries (switch,
        // close); stop rendering once we run past what fit.
        if segments.len() < i * 2 + 2 {
            break;
        }
        let label = tab_label(app);
        let active = i == tabs.active;
        let style = if active {
            theme.selection_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        };
        spans.push(Span::styled(format!(" {label} "), style));
        spans.push(Span::styled("×", Style::default().fg(theme.dim)));
        spans.push(Span::raw(" "));
    }
    f.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
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
