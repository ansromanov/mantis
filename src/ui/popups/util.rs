//! Shared layout and rendering helpers for popup overlays.
//!
//! Home to `centered_rect`, which computes a `Rect` centered within a
//! parent area from percentage width and height, splitting the space with
//! ratatui `Layout` constraints. Every centered popup (search, history, theme,
//! command palette, help, about) uses it so they share a consistent size and
//! position model. Keeping this geometry in one place means a change to how
//! popups are centered applies everywhere at once. Also owns
//! `search_toggle_spans`, the `[Aa] [\b] [.*]` search-option indicator row
//! shared by the search overlay and the in-file search bar.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme::Theme;

/// Returns a `Rect` centered in `area` using the given percentage widths.
/// Used by all popup overlays (search, history, theme, help).
pub(crate) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let margin_y = (100 - percent_y) / 2;
    let margin_x = (100 - percent_x) / 2;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(margin_y),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(margin_y),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(margin_x),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(margin_x),
        ])
        .split(vert[1])[1]
}

/// Renders the `[Aa] [\b] [.*]` search-option indicators (case-sensitive,
/// whole-word, regex). Active toggles show bold accent, inactive ones dim.
/// Shared by the search overlay and the in-file search bar.
pub(crate) fn search_toggle_spans(
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let style_for = |on: bool| {
        if on {
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.dim)
        }
    };
    vec![
        Span::styled("[Aa]", style_for(case_sensitive)),
        Span::raw(" "),
        Span::styled(r"[\b]", style_for(whole_word)),
        Span::raw(" "),
        Span::styled("[.*]", style_for(regex)),
    ]
}

#[cfg(test)]
#[path = "util_test.rs"]
mod tests;
