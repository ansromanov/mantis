use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::command_palette::COMMANDS;

use super::util::centered_rect;

pub(crate) fn draw_command_palette(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(picker) = app.command_palette.as_ref() else {
        return;
    };

    let palette_key = app
        .keys()
        .command_palette
        .first()
        .map(|b| b.display())
        .unwrap_or_else(|| "Ctrl+P".to_string());
    let title = format!(" Commands - {} ", palette_key);

    let theme = &app.theme;
    let popup = centered_rect(56, 65, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
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
            Span::styled("|", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("-".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .map(|&i| {
            let cmd = &COMMANDS[i];
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", cmd.name), Style::default().fg(theme.text)),
                if picker.binding_labels[i].is_empty() {
                    Span::raw("")
                } else {
                    Span::styled(
                        format!("[{}]", picker.binding_labels[i]),
                        Style::default().fg(theme.dim),
                    )
                },
            ]))
        })
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

    app.command_palette_area = parts[2];
    app.command_palette_offset = state.offset();
}
