//! The theme-picker popup.
//!
//! `draw_theme` renders the theme selector as a centered overlay: a query input
//! and a fuzzy-filtered list of available theme names with the selection
//! highlighted. It reads the live `ThemePicker` state from `App` and mirrors the
//! other list-style pickers. Theme names come from the built-in presets plus any
//! user overrides. Applying a highlighted theme (which reopens the current file
//! so syntax highlighting refreshes) is handled in the key/command layers; this
//! module only draws the chooser. A no-op when the picker is closed.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

pub(crate) fn draw_theme(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(picker) = app.theme_picker.as_ref() else {
        return;
    };

    let popup = centered_rect(44, 55, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Theme ")
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
        .map(|&i| ListItem::new(picker.names[i].as_str()))
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if picker.results_len() > 0 {
        state.select(Some(picker.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.theme_area = parts[2];
    app.theme_offset = state.offset();
}

#[cfg(test)]
#[path = "theme_test.rs"]
mod tests;
