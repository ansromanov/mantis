//! Mouse event dispatch for `App`.
//!
//! `handle_mouse` mirrors the keyboard overlay precedence chain for pointer
//! input: theme picker, plugin picker, command palette, history, recent files,
//! and search overlays intercept clicks first, then events route to the tree or
//! content panel by hit-testing the `Rect`s recorded during the last render. It
//! handles left-click selection, drag-selection of text, splitter dragging to
//! resize the tree pane, scrollbar dragging, wheel scrolling, and double-click
//! to open a picker result. All coordinate math must account for the panels'
//! scroll offsets, which are also captured at render time.

use std::time::{Duration, Instant};

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::list_picker::ListPicker;
use crate::selection::TextSelection;

use super::content_pos::WHEEL_STEP;
use super::{rect_contains, App, Focus};

/// Outcome of a picker mouse event handled by [`handle_picker_mouse`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum PickerMouseAction {
    /// Double-click on an item — caller should activate the selection.
    Activate,
    /// No special action; the event was handled or irrelevant.
    None,
}

/// Shared mouse-event handling for any `ListPicker` overlay.
///
/// Handles left-click (select, double-click to activate, close on click outside),
/// and scroll-wheel navigation. Returns [`PickerMouseAction::Activate`] when the
/// caller should invoke the picker-specific activation method.
fn handle_picker_mouse<P: ListPicker>(
    ev: MouseEvent,
    area: Rect,
    offset: usize,
    picker: &mut Option<P>,
    last_click: &mut Option<(Instant, usize)>,
) -> PickerMouseAction {
    match ev.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            if !rect_contains(area, ev.column, ev.row) {
                *picker = None;
                return PickerMouseAction::None;
            }
            let index = offset + (ev.row - area.y) as usize;
            let in_range = picker.as_ref().is_some_and(|p| index < p.results_len());
            if !in_range {
                return PickerMouseAction::None;
            }
            if let Some(p) = picker.as_mut() {
                p.set_selected(index);
            }
            let now = Instant::now();
            let double = matches!(
                last_click,
                Some((t, i)) if *i == index && now.duration_since(*t) < Duration::from_millis(400)
            );
            if double {
                *last_click = None;
                PickerMouseAction::Activate
            } else {
                *last_click = Some((now, index));
                PickerMouseAction::None
            }
        }
        MouseEventKind::ScrollDown => {
            if let Some(p) = picker.as_mut() {
                if p.selected() + 1 < p.results_len() {
                    p.set_selected(p.selected() + 1);
                }
            }
            PickerMouseAction::None
        }
        MouseEventKind::ScrollUp => {
            if let Some(p) = picker.as_mut() {
                p.set_selected(p.selected().saturating_sub(1));
            }
            PickerMouseAction::None
        }
        _ => PickerMouseAction::None,
    }
}

#[cfg(test)]
#[path = "mouse_handlers_test.rs"]
mod tests;

impl App {
    /// Dispatches a mouse event. Overlays (theme, plugin, command palette,
    /// history, recent files, search) intercept first; otherwise routes based
    /// on click location (tree vs content panel). Handles left-click, drag,
    /// scroll, and double-click for search/history/theme results.
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        if self.show_help {
            match ev.kind {
                MouseEventKind::ScrollDown => {
                    self.help_scroll = self.help_scroll.saturating_add(WHEEL_STEP);
                }
                MouseEventKind::ScrollUp => {
                    self.help_scroll = self.help_scroll.saturating_sub(WHEEL_STEP);
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if !rect_contains(self.help_area, ev.column, ev.row) {
                        self.show_help = false;
                        self.help_scroll = 0;
                        self.help_tab = 0;
                        return;
                    }
                    if ev.row == self.help_area.y + 1 {
                        let ranges = crate::ui::popups::help_tab_ranges(self.help_area.x + 1);
                        for (i, (start, end)) in ranges.iter().enumerate() {
                            if ev.column >= *start && ev.column < *end {
                                if self.help_tab != i {
                                    self.help_tab = i;
                                    self.help_scroll = 0;
                                }
                                return;
                            }
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_mouse(ev);
            return;
        }
        if self.plugin_picker.is_some() {
            self.handle_plugin_mouse(ev);
            return;
        }
        if self.command_palette.is_some() {
            self.handle_command_palette_mouse(ev);
            return;
        }
        if self.history.is_some() {
            self.handle_history_mouse(ev);
            return;
        }
        if self.recent_files.is_some() {
            self.handle_recent_mouse(ev);
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
                // Breadcrumb double-click: navigate on second click within 400 ms.
                let clicked = self
                    .breadcrumb_areas
                    .iter()
                    .find(|(_, area)| rect_contains(*area, ev.column, ev.row))
                    .map(|(path, _)| path.clone());
                if let Some(path) = clicked {
                    let now = Instant::now();
                    let is_double = matches!(
                        &self.last_breadcrumb_click,
                        Some((t, p))
                            if *p == path
                                && now.duration_since(*t) < Duration::from_millis(400)
                    );
                    if is_double {
                        self.last_breadcrumb_click = None;
                        self.focus = Focus::Tree;
                        self.clear_selection();
                        self.navigate_to_breadcrumb(&path);
                    } else {
                        self.last_breadcrumb_click = Some((now, path));
                    }
                    return;
                }
                if rect_contains(self.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
                    self.clear_selection();
                    let row = (ev.row - self.tree_area.y) as usize;
                    let index = self.tree_offset + row;
                    // When the inline tree filter is active, map the click
                    // through the visible-indices array to get the global node
                    // index, then close the filter (accept the selection).
                    if self.tree_filter.is_some() {
                        if let Some(ref vis) = self.tree_visible_indices {
                            if index < vis.len() {
                                let global = vis[index];
                                self.tree_selected = global;
                                self.tree_filter = None;
                                self.activate_selected();
                            }
                        }
                    } else if let Some(node) = self.nodes.get(index) {
                        self.tree_selected = index;
                        let now = Instant::now();
                        let is_dir = node.is_dir;
                        let double = matches!(
                            self.last_click,
                            Some((t, i))
                                if i == index
                                    && now.duration_since(t) < Duration::from_millis(400)
                        );
                        if double && is_dir {
                            self.last_click = None;
                            self.descend_to_selected();
                        } else {
                            self.last_click = Some((now, index));
                            self.activate_selected();
                        }
                    }
                } else if rect_contains(self.content_area, ev.column, ev.row) {
                    self.focus = Focus::Content;
                    // Check if click is on the blame column.
                    let rel_col = (ev.column.saturating_sub(self.content_area.x)) as usize;
                    if self.blame_col_width > 0 && rel_col < self.blame_col_width {
                        let pos = self.content_pos(ev.column, ev.row);
                        self.set_active_line_from_physical(pos.0);
                        self.show_line_blame = true;
                        return;
                    }
                    let on_scrollbar = self.show_scrollbar
                        && self.display_line_count() > self.content_area.height as usize
                        && ev.column
                            == self.content_area.x + self.content_area.width.saturating_sub(1);
                    if on_scrollbar {
                        self.scrollbar_drag = true;
                        self.set_scroll_from_mouse_y(ev.row);
                        self.mark_content_scrolled();
                    } else {
                        // Check if click is on the fold gutter (after the blame column).
                        let fold_gw = self.fold_gutter_width();
                        let fold_start = self.blame_col_width;
                        if fold_gw > 0 && rel_col >= fold_start && rel_col < fold_start + fold_gw {
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
                        let can_select = self.has_text_cursor();
                        if can_select {
                            let pos = self.content_pos(ev.column, ev.row);
                            self.set_active_line_from_physical(pos.0);
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
                        self.set_content_scroll(self.content_scroll.saturating_sub(1));
                    } else if ev.row >= ca.y + ca.height.saturating_sub(2) {
                        self.set_content_scroll(self.content_scroll.saturating_add(1));
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
                    self.config.tree.width = self.tree_width;
                    self.save_config();
                }
                self.scrollbar_drag = false;
                if self.drag_start.is_some() {
                    if let Some(sel) = &self.selection {
                        if !sel.is_empty() {
                            let text = self.selection_text();
                            if !text.is_empty() {
                                self.copy_to_clipboard(text, "selection");
                            }
                        }
                    }
                    self.drag_start = None;
                }
            }
            MouseEventKind::ScrollDown => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    self.set_content_scroll(self.content_scroll.saturating_add(WHEEL_STEP));
                } else if rect_contains(self.tree_area, ev.column, ev.row) {
                    self.tree_scroll = (self.tree_scroll + WHEEL_STEP).min(self.tree_scroll_max());
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.content_area, ev.column, ev.row) {
                    self.set_content_scroll(self.content_scroll.saturating_sub(WHEEL_STEP));
                } else if rect_contains(self.tree_area, ev.column, ev.row) {
                    self.tree_scroll = self.tree_scroll.saturating_sub(WHEEL_STEP);
                }
            }
            _ => {}
        }
        if self.content_scroll != scroll_before {
            self.mark_content_scrolled();
            self.mark_session_dirty();
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
    /// Close on click outside the popup area.
    fn handle_history_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.history_area,
            self.history_offset,
            &mut self.history,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.show_selected_revision();
        }
    }

    /// Handles mouse events on the theme picker overlay: click to select,
    /// double-click to apply, scroll to navigate.
    /// Close on click outside the popup area.
    fn handle_theme_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.theme_area,
            self.theme_offset,
            &mut self.theme_picker,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.apply_selected_theme();
        }
    }

    /// Handles mouse events on the command palette overlay: click to select,
    /// double-click to execute, scroll to navigate.
    /// Close on click outside the popup area.
    fn handle_command_palette_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.command_palette_area,
            self.command_palette_offset,
            &mut self.command_palette,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.dispatch_command();
        }
    }

    /// Handles mouse events on the plugin picker overlay: close on click
    /// outside the popup area.
    fn handle_plugin_mouse(&mut self, ev: MouseEvent) {
        if let MouseEventKind::Down(MouseButton::Left) = ev.kind {
            if !rect_contains(self.plugin_picker_area, ev.column, ev.row) {
                self.plugin_picker = None;
            }
        }
    }

    /// Handles mouse events on the search overlay: click to select,
    /// double-click to open the result, scroll to navigate.
    /// Close on click outside the popup area.
    fn handle_search_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.search_area,
            self.search_offset,
            &mut self.search,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.activate_search_selection();
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
        self.set_content_scroll(y * scroll_range / track_range);
    }
}
