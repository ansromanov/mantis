use std::time::{Duration, Instant};

use arboard::Clipboard;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use crate::selection::TextSelection;

use super::{rect_contains, App, Focus};

impl App {
    /// Dispatches a mouse event. Overlays (theme, history, search) intercept
    /// first; otherwise routes based on the click location (tree vs content
    /// panel). Handles left-click, drag, scroll, and double-click for
    /// search/history/theme results.
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        if self.show_help {
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_mouse(ev);
            return;
        }
        if self.history.is_some() {
            self.handle_history_mouse(ev);
            return;
        }
        if self.search.is_some() {
            self.handle_search_mouse(ev);
            return;
        }
        let scroll_before = self.content_scroll;
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
                    self.clear_selection();
                    let row = (ev.row - self.tree_area.y) as usize;
                    let index = self.tree_offset + row;
                    if index < self.nodes.len() {
                        self.tree_selected = index;
                        self.activate_selected();
                    }
                } else if rect_contains(self.content_area, ev.column, ev.row) {
                    self.focus = Focus::Content;
                    let can_select = !(self.is_diff || self.word_wrap);
                    if can_select {
                        let pos = self.content_pos(ev.column, ev.row);
                        self.drag_start = Some(pos);
                        self.selection = None;
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(start) = self.drag_start {
                    // Auto-scroll before computing position so that selection.active
                    // is calculated with the already-updated scroll offset.
                    let ca = self.content_area;
                    if ev.row < ca.y + 2 {
                        self.content_scroll = self.content_scroll.saturating_sub(1);
                    } else if ev.row >= ca.y + ca.height.saturating_sub(2) {
                        let max = self.content_line_count().saturating_sub(1);
                        self.content_scroll = (self.content_scroll + 1).min(max);
                    }
                    let pos = self.content_pos(ev.column, ev.row);
                    self.selection = Some(TextSelection {
                        anchor: start,
                        active: pos,
                    });
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.drag_start.is_some() {
                    if let Some(sel) = &self.selection {
                        if !sel.is_empty() {
                            let text = self.selection_text();
                            if !text.is_empty() {
                                if let Ok(mut cb) = Clipboard::new() {
                                    let _ = cb.set_text(text);
                                }
                            }
                        }
                    }
                    self.drag_start = None;
                }
            }
            MouseEventKind::ScrollDown => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    let max = self.content_line_count().saturating_sub(1);
                    self.content_scroll = (self.content_scroll + 3).min(max);
                } else if rect_contains(self.tree_area, ev.column, ev.row)
                    && self.tree_selected + 1 < self.nodes.len()
                {
                    self.tree_selected += 1;
                    self.try_open_selected();
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    self.content_scroll = self.content_scroll.saturating_sub(3);
                } else if rect_contains(self.tree_area, ev.column, ev.row) && self.tree_selected > 0
                {
                    self.tree_selected -= 1;
                    self.try_open_selected();
                }
            }
            _ => {}
        }
        if self.content_scroll != scroll_before {
            self.mark_content_scrolled();
        }
    }

    /// Handles mouse events on the git-history overlay: click to select,
    /// double-click to open the diff, scroll to navigate.
    fn handle_history_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.history_area, ev.column, ev.row) {
                    return;
                }
                let index = self.history_offset + (ev.row - self.history_area.y) as usize;
                let in_range = self
                    .history
                    .as_ref()
                    .is_some_and(|h| index < h.results_len());
                if !in_range {
                    return;
                }
                if let Some(h) = &mut self.history {
                    h.selected = index;
                }
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.show_selected_revision();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(h) = &mut self.history {
                    if h.selected + 1 < h.results_len() {
                        h.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(h) = &mut self.history {
                    h.selected = h.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    /// Handles mouse events on the theme picker overlay: click to select,
    /// double-click to apply, scroll to navigate.
    fn handle_theme_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.theme_area, ev.column, ev.row) {
                    return;
                }
                let index = self.theme_offset + (ev.row - self.theme_area.y) as usize;
                let in_range = self
                    .theme_picker
                    .as_ref()
                    .is_some_and(|p| index < p.results_len());
                if !in_range {
                    return;
                }
                if let Some(p) = &mut self.theme_picker {
                    p.selected = index;
                }
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.apply_selected_theme();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(p) = &mut self.theme_picker {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(p) = &mut self.theme_picker {
                    p.selected = p.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    /// Handles mouse events on the search overlay: click to select,
    /// double-click to open the result, scroll to navigate.
    fn handle_search_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.search_area, ev.column, ev.row) {
                    return;
                }
                let index = self.search_offset + (ev.row - self.search_area.y) as usize;
                let in_range = self
                    .search
                    .as_ref()
                    .is_some_and(|s| index < s.results_len());
                if !in_range {
                    return;
                }
                if let Some(s) = &mut self.search {
                    s.selected = index;
                }
                // A second click on the same row within the window opens it.
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.activate_search_selection();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(s) = &mut self.search {
                    if s.selected + 1 < s.results_len() {
                        s.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(s) = &mut self.search {
                    s.selected = s.selected.saturating_sub(1);
                }
            }
            _ => {}
        }
    }
}
