//! The revision-picker overlay for the compare-against-revision action.
//!
//! `draw_revision_picker` renders a centered overlay listing selectable
//! revisions — recent commits, local branches, tags, and shortcuts — with
//! a fuzzy-filter query bar at the top. It replaces the old blind-input
//! bar (`compare_input`) with a browsable, picker-style UX consistent with
//! the history, theme, and recent-files overlays. Free-form entry is still
//! available: when the filtered list is empty but the query is non-empty,
//! pressing Enter uses the typed text as a raw revspec.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

pub(crate) fn draw_revision_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(picker) = app.revision_picker.as_ref() else {
        return;
    };

    let popup = centered_rect(60, 60, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Select revision ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(picker.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .filter_map(|&i| picker.items.get(i))
        .map(|item| ListItem::new(Line::from(Span::raw(item.display.as_str()))))
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if picker.results_len() > 0 {
        state.select(Some(picker.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.revision_picker_area = parts[2];
    app.revision_picker_offset = state.offset();
}

#[cfg(test)]
#[path = "revision_test.rs"]
mod tests;
