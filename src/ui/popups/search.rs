//! The file/content search popup.
//!
//! `draw_search` renders the main fuzzy search overlay: a query input plus a
//! scored, filtered result list, with a `SearchMode` indicator showing whether
//! it is searching file paths or file contents (Tab toggles between them).
//! Content-mode results show the matching line and surrounding context. It reads
//! the live `SearchState` from `App` and shares its centered list-picker layout
//! with the history, command, and theme popups. Rendering only - matching,
//! scoring, and opening a result are handled by the search engine and key
//! handlers.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::search::SearchMode;

use super::util::centered_rect;

pub(crate) fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(search) = app.search.as_ref() else {
        return;
    };

    let popup = centered_rect(72, 75, area);
    f.render_widget(Clear, popup);

    let scope = if search.scoped {
        " (changed files)"
    } else {
        ""
    };
    let mode_label = match search.mode {
        SearchMode::Files => format!(" Search: Files{scope}  [Tab → Content] "),
        SearchMode::Content => format!(" Search: Content{scope}  [Tab → Files] "),
    };

    let block = Block::default()
        .title(mode_label)
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let query_parts = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(30)])
        .split(parts[0]);

    let content_min_chars = if search.regex || search.whole_word {
        1
    } else {
        2
    };
    let hint =
        if search.mode == SearchMode::Content && search.query.chars().count() < content_min_chars {
            if content_min_chars == 1 {
                "  (type 1+ char)"
            } else {
                "  (type 2+ chars)"
            }
        } else {
            ""
        };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(search.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
            Span::styled(hint, Style::default().fg(theme.dim)),
        ])),
        query_parts[0],
    );

    let mut toggle_spans = super::util::search_toggle_spans(
        search.case_sensitive,
        search.whole_word,
        search.regex,
        theme,
    );
    let total = search.results_len();
    let current = if total > 0 { search.selected + 1 } else { 0 };
    toggle_spans.push(Span::styled(
        format!("  {}/{}", current, total),
        Style::default().fg(theme.dim),
    ));

    f.render_widget(
        Paragraph::new(Line::from(toggle_spans)).alignment(ratatui::layout::Alignment::Right),
        query_parts[1],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = match search.mode {
        SearchMode::Files => search
            .file_results
            .iter()
            .map(|p| {
                let display = p.strip_prefix(&app.root).unwrap_or(p);
                ListItem::new(display.to_string_lossy().to_string())
            })
            .collect(),
        SearchMode::Content => search
            .content_results
            .iter()
            .map(|m| {
                let file = m.path.strip_prefix(&app.root).unwrap_or(&m.path);
                let trimmed = m.line.trim();
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        format!("{}:{}: ", file.display(), m.line_num),
                        Style::default().fg(theme.accent),
                    ),
                    Span::raw(trimmed),
                ])];
                for ctx_line in &m.context {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default().fg(theme.dim)),
                        Span::styled(ctx_line.trim().to_string(), Style::default().fg(theme.dim)),
                    ]));
                }
                ListItem::new(lines)
            })
            .collect(),
    };

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if search.results_len() > 0 {
        state.select(Some(search.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.search_area = parts[2];
    app.search_offset = state.offset();
}

#[cfg(test)]
#[path = "search_test.rs"]
mod tests;
