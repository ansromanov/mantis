//! Rendering and hit-test geometry for the optional action menu bar.
//!
//! The row and dropdown are projections of `ActionSpec::menu`; every frame
//! bounds work to the current menu's actions and records both rectangles for
//! pointer interaction. `menu_index_at` shares the row's column ranges with
//! the mouse handler. Labels come from the same registry as the command
//! palette, key hints use the live `Keymap`, and unavailable actions receive
//! the theme's dim color. Dropdown contents are clipped to the terminal and
//! keep the selected action visible when a menu is taller than the viewport.
//! The menu row is drawn only when configured or temporarily opened with F10,
//! leaving the default layout unchanged.

use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::{
    app::{
        menu_bar::{dropdown_scroll_offset, menu_actions, MenuBarState, MENUS},
        App,
    },
    theme::Theme,
};

/// Returns half-open column ranges for the menu labels in the menu row.
pub(crate) fn menu_ranges(area: Rect) -> Vec<(u16, u16)> {
    let mut x = area.x;
    MENUS
        .iter()
        .map(|menu| {
            let start = x;
            let end = x.saturating_add(menu.len() as u16 + 2);
            let range = (start, end.min(area.right()));
            x = end;
            x = x.saturating_add(2);
            range
        })
        .collect()
}

/// Finds the menu whose label contains the given screen column.
pub(crate) fn menu_index_at(area: Rect, column: u16) -> Option<usize> {
    if column < area.x || column >= area.right() {
        return None;
    }
    menu_ranges(area)
        .iter()
        .position(|(start, end)| column >= *start && column < *end)
}

pub(crate) fn draw_menu_bar(f: &mut Frame, app: &mut App, row: Rect) {
    app.menu_bar_area = row;
    let active = app.menu_bar_state.map(|state| state.menu_index);
    let mut spans = Vec::new();
    for (index, menu) in MENUS.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", Style::default().fg(app.theme.dim)));
        }
        let label = format!(" {menu} ");
        let style = if active == Some(index) {
            Style::default()
                .fg(app.theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::REVERSED)
        } else {
            Style::default().fg(app.theme.text)
        };
        spans.push(Span::styled(label, style));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Left),
        row,
    );
    app.menu_dropdown_area = Rect::default();
    if let Some(state) = app.menu_bar_state {
        draw_dropdown(f, app, row, state);
    }
}

fn draw_dropdown(f: &mut Frame, app: &mut App, row: Rect, state: MenuBarState) {
    let Some(&name) = MENUS.get(state.menu_index) else {
        return;
    };
    let items = menu_actions(app, name);
    if items.is_empty() || row.width < 4 || row.bottom() >= f.area().bottom() {
        return;
    }
    let theme: &Theme = &app.theme;
    let longest = items
        .iter()
        .map(|item| {
            item.label.chars().count()
                + if item.binding.is_empty() {
                    0
                } else {
                    item.binding.chars().count() + 3
                }
        })
        .max()
        .unwrap_or(8);
    let width = (longest as u16 + 4)
        .max(1)
        .min(row.width)
        .min(f.area().width);
    let available_height = f.area().bottom().saturating_sub(row.bottom());
    if available_height < 3 {
        return;
    }
    let height = (items.len() as u16 + 2).min(available_height);
    let x = menu_ranges(row)
        .get(state.menu_index)
        .map(|(start, _)| *start)
        .unwrap_or(row.x)
        .min(f.area().right().saturating_sub(width));
    let popup = Rect {
        x,
        y: row.bottom(),
        width,
        height,
    };
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let visible_rows = inner.height as usize;
    let selected = state.selected.min(items.len().saturating_sub(1));
    let offset = dropdown_scroll_offset(selected, visible_rows);
    let list_items: Vec<_> = items
        .iter()
        .skip(offset)
        .take(visible_rows)
        .map(|item| {
            let text = if item.binding.is_empty() {
                item.label.to_string()
            } else {
                format!("{}   {}", item.label, item.binding)
            };
            let style = if item.applicable {
                Style::default().fg(theme.text)
            } else {
                Style::default().fg(theme.dim)
            };
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();
    let mut list_state = ListState::default();
    list_state.select(Some(selected.saturating_sub(offset)));
    let list =
        List::new(list_items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));
    f.render_stateful_widget(list, inner, &mut list_state);
    app.menu_dropdown_area = popup;
}

#[cfg(test)]
#[path = "menu_bar_test.rs"]
mod tests;
