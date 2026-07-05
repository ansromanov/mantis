//! The About popup.
//!
//! `draw_about` renders a centered, bordered overlay showing the application
//! name and version, a short description, and the current release's "what's new"
//! notes pulled from `release_info::RELEASE`. When release metadata is present
//! it also hints that `o` opens the release page in a browser. It is a
//! read-only view of `App` state (theme and release info) and draws nothing when
//! invoked outside the overlay's active state; visibility is decided by the
//! caller in the UI orchestrator based on `App::show_about`.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

/// Caps how many changelog `entries` are shown within `budget` rows, reserving
/// one row for a "N more" marker whenever truncation is needed so the marker
/// itself never overruns the budget. Returns `(shown_entries, remaining_count)`.
pub(crate) fn fit_changelog<'a>(entries: &'a [&'a str], budget: usize) -> (&'a [&'a str], usize) {
    if entries.len() <= budget {
        (entries, 0)
    } else {
        let shown = budget.saturating_sub(1);
        (&entries[..shown], entries.len() - shown)
    }
}

pub(crate) fn draw_about(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(52, 75, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" About mantis — ? / Esc / q to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let dim = Style::default().fg(theme.dim);
    let accent = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(theme.text);

    let version = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.version.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let date = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.date.as_str())
        .unwrap_or("");
    let whats_new = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.whats_new.as_str())
        .unwrap_or("");
    let has_url = crate::release_info::RELEASE
        .as_ref()
        .map(|r| !r.release_url.is_empty())
        .unwrap_or(false);

    let mut rows: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  A fast terminal file browser with",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  syntax highlighting, markdown rendering,",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  fuzzy search, and mouse support.",
            text_style,
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Version:   ", dim),
            Span::styled(version, accent),
        ]),
    ];

    if !date.is_empty() {
        rows.push(Line::from(vec![
            Span::styled("  Released:  ", dim),
            Span::styled(date, text_style),
        ]));
    }

    rows.push(Line::from(vec![
        Span::styled("  License:   ", dim),
        Span::styled("GPL-3.0-or-later", text_style),
    ]));

    if let Some(ref latest) = app.new_version_available {
        rows.push(Line::from(vec![
            Span::styled("  Update:    ", dim),
            Span::styled(
                format!("{latest} is available! (run `mantis --update` to upgrade)"),
                accent,
            ),
        ]));
    }

    if !whats_new.is_empty() {
        rows.push(Line::from(""));
        rows.push(Line::from(vec![Span::styled(
            "  What's new:",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::UNDERLINED),
        )]));

        // The hint line below must always stay visible, so cap how many
        // changelog entries we render to whatever's left of `inner`'s height.
        let reserved = if has_url { 2 } else { 0 };
        let budget = (inner.height as usize)
            .saturating_sub(rows.len())
            .saturating_sub(reserved);
        let entries: Vec<&str> = whats_new.lines().collect();
        let (shown, remaining) = fit_changelog(&entries, budget);
        for line in shown {
            rows.push(Line::from(vec![Span::styled(
                format!("  {line}"),
                text_style,
            )]));
        }
        if remaining > 0 {
            rows.push(Line::from(vec![Span::styled(
                format!("  ... ({remaining} more, see release page)"),
                dim,
            )]));
        }
    }

    if has_url {
        rows.push(Line::from(""));
        rows.push(Line::from(vec![Span::styled(
            "  o  open release in browser     Enter/Esc/q  close",
            dim,
        )]));
    }

    f.render_widget(Paragraph::new(rows), inner);
}

#[cfg(test)]
#[path = "about_test.rs"]
mod tests;
