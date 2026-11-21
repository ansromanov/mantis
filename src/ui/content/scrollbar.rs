//! Transient content-pane scrollbar overlay.
//!
//! `draw_content_scrollbar` paints a thin scrollbar thumb on the pane's right
//! border column, sized and positioned from the current scroll offset and total
//! line count. Drawing on the border (rather than over the last text column)
//! means it never hides or shifts text, so the content does not appear to
//! change width or lose its last character when the bar fades in and out. It is
//! deliberately transient: it appears only for a short fade window
//! (`SCROLLBAR_FADE`) after the user last scrolled, tracked via
//! `App::content_scrolled_at`. Whether it shows at all is gated by the user's
//! `scrollbar` config option, so it can be disabled entirely.

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

/// Draws the transient scrollbar thumb on the content pane's right border.
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

    // Only the thumb is drawn; the rest of the border keeps its own glyph, so
    // the track needs no cells of its own.
    let lines: Vec<Line> = (0..thumb_size)
        .map(|_| Line::from(Span::styled("█", Style::default().fg(app.theme.dim))))
        .collect();

    f.render_widget(
        Paragraph::new(lines),
        Rect {
            // `inner_x + inner_w` is the border column just right of the text.
            x: inner_x + inner_w as u16,
            y: inner_y + thumb_start as u16,
            width: 1,
            height: thumb_size as u16,
        },
    );
}

#[cfg(test)]
#[path = "scrollbar_test.rs"]
mod tests;
