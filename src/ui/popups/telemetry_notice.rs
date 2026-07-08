//! The telemetry notice popup.
//!
//! Renders a centered dialog notice shown on first telemetry enable.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

pub(crate) fn draw_telemetry_notice(f: &mut Frame, app: &App, area: Rect) {
    if !app.show_telemetry_notice {
        return;
    }
    let theme = &app.theme;

    // Centered modal
    let popup = centered_rect(65, 45, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Telemetry Enabled ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if inner.height < 4 || inner.width < 10 {
        return;
    }

    let dir = crate::session::state_dir()
        .map(|d| d.join("telemetry"))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let lines = vec![
        Line::from(vec![Span::styled(
            "Mantis collects anonymous usage events locally on your machine.",
            Style::default().fg(theme.text),
        )]),
        Line::from(vec![Span::styled(
            "Nothing is ever sent to any remote server.",
            Style::default().fg(theme.text),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Storage directory: ", Style::default().fg(theme.dim)),
            Span::styled(&dir, Style::default().fg(theme.text)),
        ]),
        Line::from(vec![
            Span::styled("Complete event schema: ", Style::default().fg(theme.dim)),
            Span::styled(
                "docs/src/telemetry.md",
                Style::default().fg(theme.accent_alt),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Press ", Style::default().fg(theme.dim)),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", ", Style::default().fg(theme.dim)),
            Span::styled(
                "Esc",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", or ", Style::default().fg(theme.dim)),
            Span::styled(
                "q",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" to dismiss.", Style::default().fg(theme.dim)),
        ]),
    ];

    f.render_widget(
        Paragraph::new(lines).style(Style::default().fg(theme.text).bg(theme.background)),
        inner,
    );
}

#[cfg(test)]
#[path = "telemetry_notice_test.rs"]
mod tests;
