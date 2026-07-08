//! The first-run welcome overlay.
//!
//! `draw_welcome` renders a centered, bordered popup on the very first launch
//! (before any session/config exists). It shows the ~5 essential keybindings
//! resolved from the live action registry so they reflect user-customised
//! bindings. `Esc` dismisses; once dismissed the overlay is
//! never shown again (tracked via the global `welcome_shown.flag`).
//!
//! The overlay reads from `app.keys().label_for_action(...)` so it shows the
//! actual configured key for each action, not a hardcoded label.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

/// The action ids displayed in the welcome overlay, in order.
const WELCOME_ACTIONS: &[(&str, &str)] = &[
    ("tree_expand", "Open a file or directory"),
    ("search_files", "Filter the tree by name"),
    ("command_palette", "Find files & run commands"),
    ("help", "Open keybinding help"),
    ("quit", "Exit mantis"),
];

pub(crate) fn draw_welcome(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(60, 70, area);
    app.welcome_area = popup;
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Welcome to mantis! ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let dim = Style::default().fg(theme.dim);
    let text_style = Style::default().fg(theme.text);
    let key_style = Style::default()
        .fg(theme.accent_alt)
        .add_modifier(Modifier::BOLD);

    let mut rows: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  A fast terminal file browser with",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  syntax highlighting, fuzzy search,",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  markdown rendering, and mouse support.",
            text_style,
        )]),
        Line::from(""),
        Line::from(vec![Span::styled("  Here are the essentials:", text_style)]),
        Line::from(""),
    ];

    for &(action_id, description) in WELCOME_ACTIONS {
        let bind_label = app.keys().label_for_action(action_id);
        rows.push(Line::from(vec![
            Span::styled("  ", dim),
            Span::styled(
                if bind_label.is_empty() {
                    String::new()
                } else {
                    format!(" {bind_label:<12}")
                },
                key_style,
            ),
            Span::styled("  ", dim),
            Span::styled(description, text_style),
        ]));
    }

    rows.push(Line::from(""));
    rows.push(Line::from(vec![Span::styled(
        "  Esc     Dismiss this message",
        dim,
    )]));

    f.render_widget(Paragraph::new(rows), inner);
}

#[cfg(test)]
#[path = "welcome_test.rs"]
mod tests;
