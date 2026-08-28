//! Worktree switcher popup rendering.
//!
//! The popup presents each git worktree as a compact, fuzzy-filterable row.
//! Rendering records the list rectangle and scroll offset on `App`, allowing
//! the shared mouse picker handler to use the same geometry as the keyboard UI.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

pub(crate) fn draw_worktree_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(picker) = app.worktree_picker.as_ref() else {
        return;
    };
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .title(" Worktrees ")
        .borders(Borders::ALL)
        .style(Style::default().bg(app.theme.background))
        .border_style(Style::default().fg(app.theme.accent_alt));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Worktree",
                Style::default()
                    .fg(app.theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  {}/{}",
                picker.selected.saturating_add(1),
                picker.filtered.len()
            )),
        ])),
        parts[0],
    );
    f.render_widget(Paragraph::new(format!("> {}█", picker.query)), parts[1]);
    f.render_widget(
        Paragraph::new("-".repeat(inner.width as usize)).style(Style::default().fg(app.theme.dim)),
        parts[2],
    );
    let items = picker
        .filtered
        .iter()
        .filter_map(|&i| picker.items.get(i))
        .map(|item| ListItem::new(item.display()))
        .collect::<Vec<_>>();
    let list =
        List::new(items).highlight_style(app.theme.selection_style().add_modifier(Modifier::BOLD));
    let mut state = ListState::default();
    if !picker.filtered.is_empty() {
        state.select(Some(picker.selected));
    }
    f.render_stateful_widget(list, parts[3], &mut state);
    app.worktree_picker_area = parts[3];
    app.worktree_picker_offset = state.offset();
}

#[cfg(test)]
#[path = "worktree_test.rs"]
mod tests;
