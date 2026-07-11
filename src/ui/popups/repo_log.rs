//! The repo-wide commit log popup.
//!
//! `draw_repo_log` renders the repository commit log as a centered overlay
//! with a query input and a fuzzy-filtered, scored list of commits showing
//! short hash, date, author, and subject. The selected commit is
//! highlighted. It reads the live `RepoLogState` from `App` and shares its
//! list-picker layout and interaction model with the search, history, and
//! theme popups. Drawn only while the repo log overlay is open.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

pub(crate) fn draw_repo_log(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let Some(repo_log) = app.repo_log.as_ref() else {
        return;
    };

    let popup = centered_rect(80, 80, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Commit Log ")
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

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(repo_log.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = repo_log
        .filtered
        .iter()
        .filter_map(|&i| repo_log.commits.get(i))
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", c.short),
                    Style::default().fg(theme.accent_alt),
                ),
                Span::styled(format!("{} ", c.date), Style::default().fg(theme.accent)),
                Span::styled(format!("{} ", c.author), Style::default().fg(theme.dim)),
                Span::raw(c.subject.as_str()),
            ]))
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if repo_log.results_len() > 0 {
        state.select(Some(repo_log.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.repo_log_area = parts[2];
    app.repo_log_offset = state.offset();
}

#[cfg(test)]
#[path = "repo_log_test.rs"]
mod tests;
