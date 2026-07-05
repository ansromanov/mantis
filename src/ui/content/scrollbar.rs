//! Transient content-pane scrollbar overlay.
//!
//! `draw_content_scrollbar` paints a thin scrollbar on the right edge of the
//! content area, sized and positioned from the current scroll offset and total
//! line count. It is deliberately transient: it appears only for a short fade
//! window (`SCROLLBAR_FADE`) after the user last scrolled, tracked via
//! `App::content_scrolled_at`, so it does not permanently occupy a column. The
//! optional scroll-percentage readout is drawn alongside it. Whether it shows at
//! all is gated by the user's `scrollbar` config option, so it can be disabled
//! entirely.

use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

const SCROLLBAR_FADE: Duration = Duration::from_millis(2000);

/// Draws the transient scrollbar overlay on the right edge of the content area.
pub(crate) fn draw_content_scrollbar(
    f: &mut Frame,
    app: &App,
    inner_x: u16,
    inner_y: u16,
    inner_w: usize,
    inner_h: usize,
) {
    let total = app.display_line_count();
    if !(app.show_scrollbar
        && total > inner_h
        && inner_h > 0
        && inner_w > 0
        && app.content_scrolled_at.elapsed() < SCROLLBAR_FADE)
    {
        return;
    }
    let thumb_size = 1.max(inner_h * inner_h / total);
    let scroll_range = total - inner_h;
    let track_range = inner_h - thumb_size;
    let thumb_start = ((app.content_scroll * track_range + scroll_range / 2)
        .checked_div(scroll_range)
        .unwrap_or(0))
    .min(track_range);

    let lines: Vec<Line> = (0..inner_h)
        .map(|i| {
            if i >= thumb_start && i < thumb_start + thumb_size {
                Line::from(Span::styled("█", Style::default().fg(app.theme.dim)))
            } else {
                Line::from(Span::raw(" "))
            }
        })
        .collect();

    f.render_widget(
        Paragraph::new(lines),
        Rect {
            x: inner_x + inner_w as u16 - 1,
            y: inner_y,
            width: 1,
            height: inner_h as u16,
        },
    );
}

#[cfg(test)]
#[path = "scrollbar_test.rs"]
mod tests;
