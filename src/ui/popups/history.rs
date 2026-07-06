//! The file-history popup.
//!
//! `draw_history` renders the recently-opened-files picker as a centered overlay
//! with a query input and a fuzzy-filtered, scored list of paths, the selected
//! one highlighted. It reads the live `HistoryState` from `App` and shares its
//! list-picker layout and interaction model with the search, command, and theme
//! popups. Rendering only - building and reordering the history list and acting
//! on a selection happen in the search/state and key-handler layers. Drawn only
//! while the history overlay is open.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

pub(crate) fn draw_history(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(history) = app.history.as_ref() else {
        return;
    };

    let popup = centered_rect(72, 75, area);
    f.render_widget(Clear, popup);

    let name = history
        .file
        .strip_prefix(&app.root)
        .unwrap_or(&history.file);
    let block = Block::default()
        .title(format!(" History: {} ", name.display()))
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
            Span::raw(history.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = history
        .filtered
        .iter()
        .filter_map(|&i| history.commits.get(i))
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", c.short),
                    Style::default().fg(theme.accent_alt),
                ),
                Span::styled(format!("{} ", c.date), Style::default().fg(theme.accent)),
                Span::raw(c.subject.as_str()),
            ]))
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if history.results_len() > 0 {
        state.select(Some(history.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.history_area = parts[2];
    app.history_offset = state.offset();
}

#[cfg(test)]
#[path = "history_test.rs"]
mod tests;
