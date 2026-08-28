//! The right-click context menu popup.
//!
//! `draw_context_menu` renders the menu built by `App::open_tree_context_menu`
//! / `open_content_context_menu` anchored below-right of the click position,
//! clamped so it stays on screen. Action rows show their label with the
//! highlighted one using the theme selection style; separator rows render as
//! dim rules. The full popup `Rect` is recorded back on `App` so mouse handlers
//! can hit-test item clicks and click-away dismissal. `menu_rect` is a pure
//! geometry helper, kept separate from the painter so the popup layout is
//! unit-testable.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, ContextMenuEntry, ContextMenuState};

pub(crate) fn draw_context_menu(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(menu) = app.context_menu.as_ref() else {
        return;
    };
    let popup = menu_rect(menu, area);
    f.render_widget(Clear, popup);

    let theme = &app.theme;
    let block = Block::default()
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let items: Vec<ListItem> = menu
        .entries
        .iter()
        .map(|entry| match entry {
            ContextMenuEntry::Action { label, .. } => {
                ListItem::new(Line::from(Span::raw(label.clone())))
            }
            ContextMenuEntry::Separator => ListItem::new(Line::from(Span::styled(
                "─".repeat(inner.width.saturating_sub(2) as usize),
                Style::default().fg(theme.dim),
            ))),
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    state.select(Some(menu.selected));
    f.render_stateful_widget(list, inner, &mut state);

    app.context_menu_area = popup;
}

/// Computes the popup `Rect` for a context menu: one cell right and below the
/// anchor, sized to the longest label plus border, clamped to `area` so it
/// never spills off screen (flipping above/left of the anchor when there is
/// not enough room below/right).
fn menu_rect(menu: &ContextMenuState, area: Rect) -> Rect {
    let max_label = menu.max_label_len().max(8);
    let width = ((max_label + 4) as u16).clamp(8, area.width.saturating_sub(2).max(8));
    let height = (menu.entries.len() as u16 + 2).min(area.height).max(3);
    let max_x = area.x + area.width.saturating_sub(width);
    let max_y = area.y + area.height.saturating_sub(height);
    let x = (menu.anchor.0.saturating_add(1)).min(max_x).max(area.x);
    let y = (menu.anchor.1.saturating_add(1)).min(max_y).max(area.y);
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
#[path = "context_menu_test.rs"]
mod tests;
