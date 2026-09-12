//! Fuzzy picker rendering for open project tabs.
//!
//! The tab picker belongs to the workspace wrapper rather than an individual
//! `App`, because it switches between apps. It presents each root with its
//! available Git branch and changed-file count, and leaves search/navigation
//! state in `search::TabPicker`. The popup uses the shared centered layout
//! helper and the active tab's theme, matching the other list overlays.

use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::workspace::Tabs;

/// Draws the workspace-level fuzzy picker above the active app.
pub(crate) fn draw_tab_picker(f: &mut Frame, tabs: &mut Tabs) {
    let Some(picker) = tabs.tab_picker.as_ref() else {
        return;
    };
    let area = super::util::centered_rect(75, 60, f.area());
    tabs.tab_picker_area = area;
    let app = tabs.active_app();
    f.render_widget(Clear, area);
    let block = Block::default()
        .title(" Open tabs ")
        .borders(Borders::ALL)
        .style(Style::default().bg(app.theme.background))
        .border_style(Style::default().fg(app.theme.accent_alt));
    let inner = block.inner(area);
    f.render_widget(block, area);
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
        Paragraph::new(Line::from(format!(
            "Tab  {}/{}",
            picker.selected.saturating_add(1),
            picker.filtered.len()
        ))),
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
        .filter_map(|&index| picker.items.get(index))
        .map(|item| ListItem::new(item.display()))
        .collect::<Vec<_>>();
    let list =
        List::new(items).highlight_style(app.theme.selection_style().add_modifier(Modifier::BOLD));
    let mut state = ListState::default();
    if !picker.filtered.is_empty() {
        state.select(Some(picker.selected));
    }
    f.render_stateful_widget(list, parts[3], &mut state);
}

#[cfg(test)]
#[path = "tab_test.rs"]
mod tests;
