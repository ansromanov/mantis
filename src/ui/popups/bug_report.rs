//! The bug report popup.
//!
//! Renders a centered dialog containing a multiline text editor for the bug report body.
//! The user types the report body, submits with Ctrl+S / Ctrl+Enter, and cancels with Esc.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

pub(crate) fn draw_bug_report(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(state) = app.bug_report.as_mut() else {
        return;
    };
    let theme = &app.theme;

    // Centered modal
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Submit Bug Report ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if inner.height < 5 || inner.width < 10 {
        return;
    }

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    // Header info
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "Please describe the bug details below. Provide steps to reproduce if possible.",
            Style::default().fg(theme.text),
        )])),
        parts[0],
    );

    // Separator 1
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    // Separator 2
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[3],
    );

    // Footer info
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Ctrl+S / Ctrl+Enter: ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Submit & Save  ", Style::default().fg(theme.dim)),
            Span::styled(
                "Esc: ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Cancel", Style::default().fg(theme.dim)),
        ])),
        parts[4],
    );

    // Multiline edit area
    let edit_area = parts[2];
    state.clamp_scroll(edit_area.height as usize);

    let mut lines = Vec::new();
    let start = state.scroll_top;
    let end = (start + edit_area.height as usize).min(state.text.len());

    for i in start..end {
        let line_text = &state.text[i];
        if i == state.cursor_row {
            let char_count = line_text.chars().count();
            let col = state.cursor_col.min(char_count);
            let before: String = line_text.chars().take(col).collect();
            let after: String = line_text.chars().skip(col).collect();
            lines.push(Line::from(vec![
                Span::raw(before),
                Span::styled("█", Style::default().fg(theme.accent_alt)),
                Span::raw(after),
            ]));
        } else {
            lines.push(Line::from(vec![Span::raw(line_text.clone())]));
        }
    }

    // Fill the rest with empty rows if text is shorter than edit_area height
    while lines.len() < edit_area.height as usize {
        lines.push(Line::from(""));
    }

    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(theme.text).bg(theme.background)),
        edit_area,
    );
}

#[cfg(test)]
#[path = "bug_report_test.rs"]
mod tests;
