//! The recent-files overlay popup.
//!
//! `draw_recent` renders a centered floating overlay with a query input and a
//! fuzzy-filtered list of recently opened file paths, most-recent-first. It
//! shares the same layout and interaction model as the history, search, command,
//! and theme pickers: type to filter, Up/Down to move the cursor, Enter to open,
//! Esc to close. Rendering only; state management and key handling live in the
//! app/key-handler layers. The rendered list `Rect` and scroll offset are stored
//! back on `App` so mouse handlers can hit-test clicks.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::search::RecentFilesState;
use crate::theme::Theme;

use super::util::centered_rect;

pub(crate) fn draw_recent(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(recent) = app.recent_files.as_ref() else {
        return;
    };
    let theme = app.theme.clone();
    let root = app.root.clone();
    let (list_area, offset) = draw_path_picker(f, &theme, &root, area, "Recent files", recent);
    app.recent_area = list_area;
    app.recent_offset = offset;
}

/// Draws the bookmarks picker using the same path-picker layout as recent files.
pub(crate) fn draw_bookmarks(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(bookmarks) = app.bookmarks.as_ref() else {
        return;
    };
    let theme = app.theme.clone();
    let root = app.root.clone();
    let (list_area, offset) = draw_path_picker(f, &theme, &root, area, "Bookmarks", bookmarks);
    app.recent_area = list_area;
    app.recent_offset = offset;
}

fn draw_path_picker(
    f: &mut Frame,
    theme: &Theme,
    root: &std::path::Path,
    area: Rect,
    title: &str,
    recent: &RecentFilesState,
) -> (Rect, usize) {
    let popup = centered_rect(72, 75, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" {title} "))
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
            Span::raw(recent.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = recent
        .filtered
        .iter()
        .filter_map(|&i| recent.paths.get(i))
        .map(|p| {
            let display = p.strip_prefix(root).unwrap_or(p);
            ListItem::new(Line::from(Span::raw(
                display.to_string_lossy().into_owned(),
            )))
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if recent.results_len() > 0 {
        state.select(Some(recent.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    (parts[2], state.offset())
}

#[cfg(test)]
#[path = "recent_test.rs"]
mod tests;
