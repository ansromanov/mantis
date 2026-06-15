//! Shared layout helpers for popup overlays.
//!
//! Currently home to `centered_rect`, which computes a `Rect` centered within a
//! parent area from percentage width and height, splitting the space with
//! ratatui `Layout` constraints. Every centered popup (search, history, theme,
//! command palette, help, about) uses it so they share a consistent size and
//! position model. Keeping this geometry in one place means a change to how
//! popups are centered applies everywhere at once. Pure layout math with no
//! `App` or theme dependency.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

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
