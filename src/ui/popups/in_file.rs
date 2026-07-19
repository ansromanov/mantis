//! The in-file search bar.
//!
//! `draw_in_file_search` renders the incremental within-the-current-file search
//! as a thin bar near the bottom of the content area (not a centered popup),
//! showing the query, the current/total match count, and the search-option
//! indicators (`[Aa] [\b] [.*]`). It reads the live
//! `InFileSearch` state from `App`; the matches themselves are highlighted in
//! the content pane by `ui::content::search`, so this module draws only the
//! prompt/status line. It is a no-op when in-file search is inactive, and its
//! position tracks the content area so it sits just above the status bar.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::app::App;

pub(crate) fn draw_in_file_search(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(s) = app.in_file_search.as_ref() else {
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

    let bar_parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(14)])
        .split(bar_rect);

    let total = s.matches.len();
    let current = if total > 0 { s.current + 1 } else { 0 };
    let suffix = format!(" ({}/{})", current, total);
    let max_w = bar_parts[0].width as usize;
    let query_display: String = s
        .query
        .chars()
        .take(max_w.saturating_sub(suffix.len() + 2))
        .collect();
    let text = format!("/{}{}", query_display, suffix);
    if text.len() > max_w {
        let truncated: String = text.chars().take(max_w).collect();
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                truncated,
                Style::default().fg(theme.accent_alt).bg(theme.background),
            )])),
            bar_parts[0],
        );
    } else {
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
                Span::styled(suffix, Style::default().fg(theme.dim)),
            ])),
            bar_parts[0],
        );
    }

    let toggle_spans =
        super::util::search_toggle_spans(s.case_sensitive, s.whole_word, s.regex, theme);
    f.render_widget(
        Paragraph::new(Line::from(toggle_spans)).alignment(ratatui::layout::Alignment::Right),
        bar_parts[1],
    );
}

pub(crate) fn draw_filter_bar(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(s) = app.filter_bar.as_ref() else {
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

    let total = app.line_count();
    let visible = app.filter_display_map.len();
    let hidden = total.saturating_sub(visible);
    let suffix = format!(" (hidden: {hidden})");
    let max_w = bar_rect.width as usize;
    let query_display: String = s
        .query
        .chars()
        .take(max_w.saturating_sub(suffix.len() + 10))
        .collect();

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Filter: ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(query_display.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
            Span::styled(suffix, Style::default().fg(theme.dim)),
        ])),
        bar_rect,
    );
}

#[cfg(test)]
#[path = "in_file_test.rs"]
mod tests;
