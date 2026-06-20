//! Mouse event dispatch for `App`.
//!
//! `handle_mouse` mirrors the keyboard overlay precedence chain for pointer
//! input: theme picker, command palette, history, recent files, and search
//! overlays intercept clicks first, then events route to the tree or content
//! panel by hit-testing the `Rect`s recorded during the last render. It handles
//! left-click selection, drag-selection of text, splitter dragging to resize the
//! tree pane, scrollbar dragging, wheel scrolling, and double-click to open a
//! picker result. All coordinate math must account for the panels' scroll
//! offsets, which are also captured at render time.

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
        if self.command_palette.is_some() {
            self.handle_command_palette_mouse(ev);
            return;
        }
        if self.recent_files.is_some() {
            self.handle_recent_mouse(ev);
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
                if rect_contains(self.splitter_area, ev.column, ev.row) {
                    self.splitter_drag = true;
                    return;
                }
                // Breadcrumb click: check before the tree area so breadcrumb
                // segments take priority over the tree row that happens to lie
                // at the same screen position (breadcrumb sits above the list).
                let clicked = self
                    .breadcrumb_areas
                    .iter()
                    .find(|(_, area)| rect_contains(*area, ev.column, ev.row))
                    .map(|(path, _)| path.clone());
                if let Some(path) = clicked {
                    self.focus = Focus::Tree;
                    self.clear_selection();
                    self.navigate_to_breadcrumb(&path);
                    return;
                }
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
                    let on_scrollbar = self.show_scrollbar
                        && self.display_line_count() > self.content_area.height as usize
                        && ev.column
                            == self.content_area.x + self.content_area.width.saturating_sub(1);
                    if on_scrollbar {
                        self.scrollbar_drag = true;
                        self.set_scroll_from_mouse_y(ev.row);
                        self.mark_content_scrolled();
                    } else {
                        // Check if click is on the fold gutter (leftmost fold_gutter_width columns).
                        let fold_gw = self.fold_gutter_width();
                        let rel_col = (ev.column.saturating_sub(self.content_area.x)) as usize;
                        if fold_gw > 0 && rel_col < fold_gw {
                            // Find the fold region for this screen row.
                            let hit = self
                                .fold_gutter_rows
                                .iter()
                                .find(|&&(y, _)| y == ev.row)
                                .map(|&(_, ri)| ri);
                            if let Some(ri) = hit {
                                self.toggle_fold_region(ri);
                                self.mark_content_scrolled();
                                return;
                            }
                        }
                        let can_select = !self.is_diff;
                        if can_select {
                            let pos = self.content_pos(ev.column, ev.row);
                            self.drag_start = Some(pos);
                            self.selection = None;
                        }
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.splitter_drag {
                    let left = self.tree_area.x.saturating_sub(1);
                    let total = (self.tree_area.width + self.content_area.width + 4) as u32;
                    let col = ev.column.saturating_sub(left) as u32;
                    if let Some(pct) = (col * 100 + total / 2).checked_div(total) {
                        self.tree_width = (pct.min(100) as u16).clamp(5, 95);
                    }
                } else if self.scrollbar_drag {
                    self.set_scroll_from_mouse_y(ev.row);
                    self.mark_content_scrolled();
                } else if let Some(start) = self.drag_start {
                    // Auto-scroll before computing position so that selection.active
                    // is calculated with the already-updated scroll offset.
                    let ca = self.content_area;
                    if ev.row < ca.y + 2 {
                        self.content_scroll = self.content_scroll.saturating_sub(1);
                    } else if ev.row >= ca.y + ca.height.saturating_sub(2) {
                        self.content_scroll =
                            (self.content_scroll + 1).min(self.content_scroll_max());
                    }
                    let pos = self.content_pos(ev.column, ev.row);
                    self.selection = Some(TextSelection {
                        anchor: start,
                        active: pos,
                    });
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if self.splitter_drag {
                    self.splitter_drag = false;
                    self.config.tree_width = self.tree_width;
                    self.save_config();
                }
                self.scrollbar_drag = false;
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
                    self.content_scroll = (self.content_scroll + 3).min(self.content_scroll_max());
                } else if rect_contains(self.tree_area, ev.column, ev.row) {
                    if self.tree_independent_scroll {
                        self.tree_scroll = (self.tree_scroll + 3).min(self.tree_scroll_max());
                    } else if self.tree_selected + 1 < self.nodes.len() {
                        self.tree_selected += 1;
                        self.try_open_selected();
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    self.content_scroll = self.content_scroll.saturating_sub(3);
                } else if rect_contains(self.tree_area, ev.column, ev.row) {
                    if self.tree_independent_scroll {
                        self.tree_scroll = self.tree_scroll.saturating_sub(3);
                    } else if self.tree_selected > 0 {
                        self.tree_selected -= 1;
                        self.try_open_selected();
                    }
                }
            }
            _ => {}
        }
        if self.content_scroll != scroll_before {
            self.mark_content_scrolled();
        }
    }

    /// Handles mouse events on the recent-files overlay: click to open,
    /// outside click to close, scroll to navigate.
    fn handle_recent_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.recent_area, ev.column, ev.row) {
                    let row = (ev.row - self.recent_area.y) as usize;
                    let index = self.recent_offset + row;
                    let in_range = self
                        .recent_files
                        .as_ref()
                        .is_some_and(|r| index < r.results_len());
                    if in_range {
                        if let Some(r) = &mut self.recent_files {
                            r.selected = index;
                        }
                        self.activate_recent_selection();
                    }
                } else {
                    self.recent_files = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(r) = &mut self.recent_files {
                    r.selected = r.selected.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(r) = &mut self.recent_files {
                    if r.selected + 1 < r.results_len() {
                        r.selected += 1;
                    }
                }
            }
            _ => {}
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

    /// Handles mouse events on the command palette overlay: click to select,
    /// double-click to execute, scroll to navigate.
    fn handle_command_palette_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.command_palette_area, ev.column, ev.row) {
                    return;
                }
                let index =
                    self.command_palette_offset + (ev.row - self.command_palette_area.y) as usize;
                let in_range = self
                    .command_palette
                    .as_ref()
                    .is_some_and(|p| index < p.results_len());
                if !in_range {
                    return;
                }
                if let Some(p) = &mut self.command_palette {
                    p.selected = index;
                }
                let now = Instant::now();
                let double = matches!(
                    self.last_click,
                    Some((t, i)) if i == index && now.duration_since(t) < Duration::from_millis(400)
                );
                if double {
                    self.last_click = None;
                    self.dispatch_command();
                } else {
                    self.last_click = Some((now, index));
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(p) = &mut self.command_palette {
                    if p.selected + 1 < p.results_len() {
                        p.selected += 1;
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(p) = &mut self.command_palette {
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

    /// Maps a mouse row to a content scroll position, used for scrollbar dragging.
    pub(super) fn set_scroll_from_mouse_y(&mut self, row: u16) {
        let total = self.display_line_count();
        let inner_h = self.content_area.height as usize;
        if total <= inner_h || inner_h == 0 {
            return;
        }
        let thumb_size = (inner_h * inner_h / total).max(1);
        let scroll_range = total - inner_h;
        let track_range = inner_h.saturating_sub(thumb_size);
        if track_range == 0 {
            return;
        }
        let y = (row as usize).saturating_sub(self.content_area.y as usize);
        let y = y.min(track_range);
        self.content_scroll = (y * scroll_range / track_range).min(scroll_range);
    }
}
