//! The compare-against-revision input bar.
//!
//! `draw_compare_input` renders a thin input bar at the bottom of the content
//! area where the user types a revision (commit hash, branch name, `HEAD~3`,
//! etc.). It shows `rev: ` and the typed text with a cursor; Enter enters
//! compare mode against that revision, Esc closes. The bar sits at the same
//! position as the go-to-line bar.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::App;

pub(crate) fn draw_compare_input(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(s) = app.compare_input.as_ref() else {
        return;
    };
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
    let query_display: String = s.query.chars().take(max_w.saturating_sub(6)).collect();
    let text = format!("rev: {}{}", query_display, '█');
    if text.len() > max_w {
        let truncated: String = text.chars().take(max_w).collect();
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                truncated,
                Style::default().fg(theme.accent_alt).bg(theme.background),
            )])),
            bar_rect,
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "rev: ",
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
}

#[cfg(test)]
#[path = "compare_input_test.rs"]
mod tests;
