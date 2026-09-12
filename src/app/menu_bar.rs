//! Registry-driven menu-bar state and action lookup.
//!
//! The optional menu row groups palette actions through `ActionSpec::menu`.
//! This module owns the active dropdown, keyboard navigation, and the small
//! action-row projection used by the menu renderer and mouse handler. The
//! registry remains the source of labels and action ids, while key labels and
//! applicability are resolved from the current application state. Keyboard
//! and pointer activation converge on the same canonical action dispatcher,
//! preserving status messages, usage tracking, and action telemetry.

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::telemetry::ActionSource;

use super::{rect_contains, App};

/// Menu groups in their stable left-to-right order.
pub(crate) const MENUS: &[&str] = &[
    "General", "View", "Git", "Copy", "Navigate", "Tree", "Tabs", "Safety",
];

/// One action row projected from the registry for rendering and dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuAction {
    pub id: &'static str,
    pub label: &'static str,
    pub binding: String,
    pub applicable: bool,
}

/// State for the open menu bar and its dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuBarState {
    pub menu_index: usize,
    pub selected: usize,
}

/// Returns the actions for a menu in their registry-defined order.
pub(crate) fn menu_actions(app: &App, menu: &str) -> Vec<MenuAction> {
    let mut actions: Vec<_> = crate::actions::ACTIONS
        .iter()
        .filter_map(|action| {
            let (action_menu, order) = action.menu?;
            if action_menu != menu {
                return None;
            }
            let label = action.palette?;
            let binding = app.keys.label_for_action(action.id);
            Some((
                order,
                MenuAction {
                    id: action.id,
                    label,
                    binding: if binding.is_empty() {
                        "-".to_string()
                    } else {
                        binding
                    },
                    applicable: app.check_applicability(action.id).is_ok(),
                },
            ))
        })
        .collect();
    actions.sort_by_key(|(order, _)| *order);
    actions.into_iter().map(|(_, action)| action).collect()
}

/// Top row index needed to keep a selected action visible in a short dropdown.
pub(crate) fn dropdown_scroll_offset(selected: usize, visible_rows: usize) -> usize {
    selected.saturating_sub(visible_rows.saturating_sub(1))
}

impl App {
    /// Opens the first menu in the menu bar.
    pub(crate) fn open_menu_bar(&mut self) {
        self.menu_bar_state = Some(MenuBarState {
            menu_index: 0,
            selected: 0,
        });
    }

    /// Handles keyboard navigation while a menu dropdown is open.
    pub(crate) fn handle_menu_bar_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc || key.code == KeyCode::F(10) {
            self.menu_bar_state = None;
            return;
        }
        let Some(mut state) = self.menu_bar_state else {
            return;
        };
        match key.code {
            KeyCode::Left => {
                state.menu_index = if state.menu_index == 0 {
                    MENUS.len().saturating_sub(1)
                } else {
                    state.menu_index - 1
                };
                state.selected = 0;
            }
            KeyCode::Right => {
                state.menu_index = (state.menu_index + 1) % MENUS.len().max(1);
                state.selected = 0;
            }
            KeyCode::Up => {
                state.selected = state.selected.saturating_sub(1);
            }
            KeyCode::Down => {
                let items = menu_actions(self, MENUS[state.menu_index]);
                state.selected = (state.selected + 1).min(items.len().saturating_sub(1));
            }
            KeyCode::Enter => {
                if let Some(item) = menu_actions(self, MENUS[state.menu_index]).get(state.selected)
                {
                    let id = item.id;
                    self.menu_bar_state = None;
                    self.dispatch_action_id(id, ActionSource::Menu);
                    return;
                }
            }
            _ => {}
        }
        self.menu_bar_state = Some(state);
    }

    /// Routes pointer movement and clicks within the menu row and dropdown.
    pub(crate) fn handle_menu_bar_mouse(&mut self, event: MouseEvent) {
        let in_bar = rect_contains(self.menu_bar_area, event.column, event.row);
        let in_dropdown = rect_contains(self.menu_dropdown_area, event.column, event.row);
        if matches!(
            event.kind,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
        ) {
            if let Some(mut state) = self.menu_bar_state {
                let count = menu_actions(self, MENUS[state.menu_index]).len();
                state.selected = match event.kind {
                    MouseEventKind::ScrollDown => (state.selected + 1).min(count.saturating_sub(1)),
                    MouseEventKind::ScrollUp => state.selected.saturating_sub(1),
                    _ => state.selected,
                };
                self.menu_bar_state = Some(state);
            }
            return;
        }
        if matches!(event.kind, MouseEventKind::Moved) {
            if in_bar {
                if let Some(index) =
                    crate::ui::menu_bar::menu_index_at(self.menu_bar_area, event.column)
                {
                    self.menu_bar_state = Some(MenuBarState {
                        menu_index: index,
                        selected: 0,
                    });
                }
            } else if in_dropdown {
                if let Some(state) = self.menu_bar_state {
                    if event.row <= self.menu_dropdown_area.y
                        || event.row >= self.menu_dropdown_area.bottom().saturating_sub(1)
                    {
                        return;
                    }
                    let visible_rows = self.menu_dropdown_area.height.saturating_sub(2) as usize;
                    let row = dropdown_scroll_offset(state.selected, visible_rows)
                        + event.row.saturating_sub(self.menu_dropdown_area.y + 1) as usize;
                    let count = menu_actions(self, MENUS[state.menu_index]).len();
                    if row < count {
                        if let Some(state) = self.menu_bar_state.as_mut() {
                            state.selected = row;
                        }
                    }
                }
            } else if self.config.ui.menu_bar {
                self.menu_bar_state = None;
            }
            return;
        }
        if let MouseEventKind::Down(MouseButton::Left) = event.kind {
            if in_bar {
                if let Some(index) =
                    crate::ui::menu_bar::menu_index_at(self.menu_bar_area, event.column)
                {
                    self.menu_bar_state = Some(MenuBarState {
                        menu_index: index,
                        selected: 0,
                    });
                }
                return;
            }
            if in_dropdown {
                let Some(state) = self.menu_bar_state else {
                    return;
                };
                if event.row <= self.menu_dropdown_area.y
                    || event.row >= self.menu_dropdown_area.bottom().saturating_sub(1)
                {
                    return;
                }
                let visible_rows = self.menu_dropdown_area.height.saturating_sub(2) as usize;
                let row = dropdown_scroll_offset(state.selected, visible_rows)
                    + event.row.saturating_sub(self.menu_dropdown_area.y + 1) as usize;
                if let Some(item) = menu_actions(self, MENUS[state.menu_index]).get(row) {
                    let id = item.id;
                    self.menu_bar_state = None;
                    self.dispatch_action_id(id, ActionSource::Mouse);
                }
                return;
            }
            self.menu_bar_state = None;
        }
    }
}

#[cfg(test)]
#[path = "menu_bar_test.rs"]
mod tests;
