//! The inline tree-filter bar.
//!
//! `draw_tree_filter` renders the incremental tree-name filter as a thin bar
//! near the bottom of the tree pane (mirroring the in-file search bar in the
//! content pane). It shows the query and a block-cursor. It is a no-op when
//! the tree filter is inactive, and its position tracks the tree area so it
//! sits just above the status bar inside the tree panel.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::App;

/// Draws the inline tree-filter bar at the bottom of the tree panel area.
/// Called only when `app.tree_filter` is `Some`.
pub(crate) fn draw_tree_filter(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let filter = app.tree_filter.as_ref().unwrap();
    let bar_y = area.y + area.height.saturating_sub(2);
    let bar_rect = Rect {
        x: area.x + 1,
        y: bar_y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    if bar_rect.width < 4 {
        return;
    }
    f.render_widget(Clear, bar_rect);

    let max_w = bar_rect.width as usize;
    // Reserve 1 cell for the cursor block; "/ " prefix is 2 chars.
    let query_display: String = filter.query.chars().take(max_w.saturating_sub(3)).collect();
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "/",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(query_display.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        bar_rect,
    );
}
