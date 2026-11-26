//! Mouse event dispatch for `App`.
//!
//! `handle_mouse` mirrors the keyboard overlay precedence chain for pointer
//! input: theme picker, command palette, history, recent files,
//! and search overlays intercept clicks first, then events route to the tree or
//! content panel by hit-testing the `Rect`s recorded during the last render. It
//! handles left-click selection, drag-selection of text, splitter dragging to
//! resize the tree pane, scrollbar dragging, wheel scrolling, double-click to
//! open a picker result, and right-click to open menus on tree, content,
//! breadcrumb, blame, hunk, and status-bar targets. While the
//! context menu or action-menu row is open, pointer events route to its own
//! handler so item clicks and dismissal work without falling through to panels.
//! The status-bar Git segment opens the revision picker on its Branches tab for
//! read-only branch browsing. All coordinate math must account for the panels'
//! scroll offsets, which are also captured at render time.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::list_picker::ListPicker;
use crate::selection::TextSelection;

use super::content_pos::WHEEL_STEP;
use super::{rect_contains, App, Focus, Overlay};

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
pub(super) fn handle_picker_mouse<P: ListPicker>(
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
    /// Dispatches a mouse event. Overlays (theme, command palette,
    /// history, recent files, search) intercept first; otherwise routes based
    /// on click location (tree vs content panel). Handles left-click, drag,
    /// scroll, and double-click for search/history/theme results.
    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        self.pause_follow_for_input();
        let top = self.top_overlay();
        // Overlays that sit above the menu bar swallow its clicks; below them a
        // click on the bar (or an open dropdown) goes to the bar.
        let above_menu_bar = matches!(
            top,
            Some(Overlay::Welcome | Overlay::Help | Overlay::About | Overlay::ContextMenu)
        );
        if !above_menu_bar
            && top != Some(Overlay::MenuBar)
            && self.config.ui.menu_bar
            && rect_contains(self.geo.menu_bar_area, ev.column, ev.row)
        {
            self.handle_menu_bar_mouse(ev);
            return;
        }
        match top {
            Some(Overlay::Welcome) => return self.handle_welcome_mouse(ev),
            // The about box has no mouse interaction; it still swallows input.
            Some(Overlay::About) => return,
            Some(Overlay::Help) => return self.handle_help_mouse(ev),
            Some(Overlay::ContextMenu) => return self.handle_context_menu_mouse(ev),
            Some(Overlay::MenuBar) => return self.handle_menu_bar_mouse(ev),
            Some(Overlay::FilterBar) => return self.handle_filter_bar_mouse(ev),
            Some(Overlay::BugReport) => return self.handle_bug_report_mouse(ev),
            Some(Overlay::RevisionPicker) => return self.handle_revision_mouse(ev),
            Some(Overlay::WorktreePicker) => return self.handle_worktree_mouse(ev),
            Some(Overlay::ThemePicker) => return self.handle_theme_mouse(ev),
            Some(Overlay::CommandPalette) => return self.handle_command_palette_mouse(ev),
            Some(Overlay::History) => return self.handle_history_mouse(ev),
            Some(Overlay::RepoLog) => return self.handle_repo_log_mouse(ev),
            Some(Overlay::RecentFiles) => return self.handle_recent_mouse(ev),
            Some(Overlay::Bookmarks) => return self.handle_bookmarks_mouse(ev),
            Some(Overlay::Search) => return self.handle_search_mouse(ev),
            // Inline prompts: keep wheel events inside the prompt. Without this
            // guard a wheel over it falls through to the tree/content handlers
            // and changes the underlying view/focus.
            Some(Overlay::InFileBar) => {
                match ev.kind {
                    MouseEventKind::ScrollDown => self.in_file_search_next(),
                    MouseEventKind::ScrollUp => self.in_file_search_prev(),
                    _ => return,
                }
                return;
            }
            Some(Overlay::TreeFilter)
                if matches!(
                    ev.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                ) =>
            {
                match ev.kind {
                    MouseEventKind::ScrollDown => self.handle_tree_filter_key(KeyEvent::new(
                        KeyCode::Down,
                        KeyModifiers::empty(),
                    )),
                    MouseEventKind::ScrollUp => self
                        .handle_tree_filter_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty())),
                    _ => return,
                }
                return;
            }
            Some(Overlay::GotoLine)
                if matches!(
                    ev.kind,
                    MouseEventKind::ScrollDown | MouseEventKind::ScrollUp
                ) =>
            {
                return;
            }
            // Clicks outside the tree filter / go-to-line prompt reach the panes.
            _ => {}
        }
        // A click on a hint replays its key, so it behaves exactly like
        // pressing it (including in the focused pane and with remapped keys).
        if let MouseEventKind::Down(MouseButton::Left) = ev.kind {
            if let Some(action) = self
                .geo
                .hint_targets
                .iter()
                .find(|(rect, _)| rect_contains(*rect, ev.column, ev.row))
                .map(|(_, action)| *action)
            {
                let scope = match self.focus {
                    Focus::Tree => crate::config::BindingScope::Tree,
                    Focus::Content => crate::config::BindingScope::Content,
                };
                if let Some(key) = self
                    .keys()
                    .binding_in(action, scope)
                    .map(|binding| (*binding).to_key_event())
                {
                    self.handle_key(key);
                }
                return;
            }
        }
        if rect_contains(self.geo.statusbar_area, ev.column, ev.row) {
            match ev.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    if let Some(text) = crate::ui::statusbar::segment_at(
                        self,
                        self.geo.statusbar_area,
                        ev.column,
                        ev.row,
                    ) {
                        if text.contains("git mode") {
                            self.toggle_git_mode();
                            return;
                        } else if text.contains("diff:") {
                            self.diff_mode = self.diff_mode.next();
                            self.config.git.diff.mode = self.diff_mode;
                            self.save_config();
                            if let Some(path) = self.current_file.clone() {
                                self.show_working_tree_diff(&path);
                            }
                            return;
                        } else if text.contains("sxs") || text.contains("unified") {
                            self.diff_side_by_side = !self.diff_side_by_side;
                            self.config.git.diff.side_by_side = self.diff_side_by_side;
                            self.save_config();
                            self.content_hscroll = 0;
                            self.clamp_content_scroll();
                            return;
                        }
                    }
                    if let Some(segment) = crate::ui::statusbar::hit_test(self, ev.column, ev.row) {
                        self.activate_statusbar_segment(segment);
                    }
                    return;
                }
                MouseEventKind::Down(MouseButton::Right) => {
                    let segment = crate::ui::statusbar::segment_at(
                        self,
                        self.geo.statusbar_area,
                        ev.column,
                        ev.row,
                    )
                    .unwrap_or_default();
                    self.open_statusbar_context_menu(segment, (ev.column, ev.row));
                }
                _ => {}
            }
            return;
        }
        let scroll_before = self.content_scroll;
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.geo.splitter_area, ev.column, ev.row) {
                    self.splitter_drag = true;
                    return;
                }
                // Breadcrumb double-click: navigate on second click within 400 ms.
                let clicked = self
                    .geo
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
                if rect_contains(self.geo.diff_header_area, ev.column, ev.row) {
                    let rel_x = ev.column.saturating_sub(self.geo.diff_header_area.x);
                    if (20..36).contains(&rel_x) {
                        self.diff_side_by_side = !self.diff_side_by_side;
                        self.config.git.diff.side_by_side = self.diff_side_by_side;
                        self.save_config();
                        self.content_hscroll = 0;
                        self.clamp_content_scroll();
                    } else {
                        self.diff_mode = self.diff_mode.next();
                        self.config.git.diff.mode = self.diff_mode;
                        self.save_config();
                        if let Some(path) = self.current_file.clone() {
                            self.show_working_tree_diff(&path);
                        }
                    }
                    return;
                }
                let tree_top = self.geo.tree_area.y.saturating_sub(1);
                if ev.row == tree_top
                    && ev.column >= self.geo.tree_area.x
                    && ev.column < self.geo.tree_area.right()
                    && (self.git_info.is_some() || self.git_mode)
                {
                    self.toggle_git_mode();
                    return;
                }
                if rect_contains(self.geo.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
                    self.clear_selection();
                    let row = (ev.row - self.geo.tree_area.y) as usize;
                    let index = self.geo.tree_offset + row;

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
                } else if self.content_scrollbar_hit(ev.column, ev.row) {
                    self.focus = Focus::Content;
                    self.scrollbar_drag = true;
                    self.set_scroll_from_mouse_y(ev.row);
                    self.mark_content_scrolled();
                } else if rect_contains(self.geo.content_area, ev.column, ev.row) {
                    self.focus = Focus::Content;
                    let rel_col = (ev.column.saturating_sub(self.geo.content_area.x)) as usize;
                    // Check if click is on the fold gutter (at the start of the gutter).
                    let fold_gw = self.fold_gutter_width();
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
                    let can_select = self.can_mouse_select();
                    if can_select {
                        let pos = self.content_pos(ev.column, ev.row);
                        if self.has_text_cursor() {
                            self.set_active_line_from_physical(pos.0);
                        }
                        self.drag_start = Some(pos);
                        self.selection = None;
                    }
                } else if self.show_blame
                    && self.has_text_cursor()
                    && rect_contains(self.geo.blame_area, ev.column, ev.row)
                {
                    self.focus = Focus::Content;
                    let rel_row = (ev.row.saturating_sub(self.geo.blame_area.y)) as usize;
                    let display_line = self.content_scroll + rel_row;
                    if display_line < self.display_line_count() {
                        let phys = self.display_to_physical(display_line);
                        self.set_active_line_from_physical(phys);
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let anchor = (ev.column, ev.row);
                let breadcrumb = self
                    .geo
                    .breadcrumb_areas
                    .iter()
                    .find(|(_, area)| rect_contains(*area, ev.column, ev.row))
                    .map(|(path, _)| path.clone());
                if let Some(path) = breadcrumb {
                    self.open_breadcrumb_context_menu(path, anchor);
                } else if self.show_blame && rect_contains(self.geo.blame_area, ev.column, ev.row) {
                    let display_line = self
                        .content_scroll
                        .saturating_add(ev.row.saturating_sub(self.geo.blame_area.y) as usize);
                    if display_line < self.display_line_count() {
                        let physical = self.display_to_physical(display_line);
                        self.open_blame_context_menu(physical, anchor);
                    }
                } else if rect_contains(self.geo.tree_area, ev.column, ev.row) {
                    let row = (ev.row - self.geo.tree_area.y) as usize;
                    let index = self.geo.tree_offset + row;
                    if self.tree_filter.is_some() {
                        if let Some(ref vis) = self.tree_visible_indices {
                            if index < vis.len() {
                                let global = vis[index];
                                self.open_tree_context_menu(global, anchor);
                            }
                        }
                    } else if self.nodes.get(index).is_some() {
                        self.open_tree_context_menu(index, anchor);
                    }
                } else if rect_contains(self.geo.content_area, ev.column, ev.row) {
                    if !self.is_diff && self.has_text_cursor() {
                        let pos = self.content_pos(ev.column, ev.row);
                        self.set_active_line_from_physical(pos.0);
                    }
                    let row = self
                        .content_scroll
                        .saturating_add(ev.row.saturating_sub(self.geo.content_area.y) as usize);
                    let header_row = self
                        .is_diff
                        .then(|| self.diff_hunk_header_for_row(row))
                        .flatten();
                    if let Some(header_row) = header_row {
                        self.open_hunk_context_menu(header_row, anchor);
                    } else {
                        self.open_content_context_menu(anchor);
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if self.splitter_drag {
                    let left = self.geo.tree_area.x.saturating_sub(1);
                    let total = (self.geo.tree_area.width + self.geo.content_area.width + 4) as u32;
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
                    let ca = self.geo.content_area;
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
                if rect_contains(self.geo.content_area, ev.column, ev.row)
                    || (self.show_blame && rect_contains(self.geo.blame_area, ev.column, ev.row))
                {
                    self.focus = Focus::Content;
                    self.scroll_content_by(WHEEL_STEP as isize);
                } else if rect_contains(self.geo.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
                    self.tree_scroll = (self.tree_scroll + WHEEL_STEP).min(self.tree_scroll_max());
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.geo.content_area, ev.column, ev.row)
                    || (self.show_blame && rect_contains(self.geo.blame_area, ev.column, ev.row))
                {
                    self.focus = Focus::Content;
                    self.scroll_content_by(-(WHEEL_STEP as isize));
                } else if rect_contains(self.geo.tree_area, ev.column, ev.row) {
                    self.focus = Focus::Tree;
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

    /// Runs the existing action associated with an interactive status-bar
    /// segment. The segment geometry itself comes from the last rendered line.
    fn activate_statusbar_segment(&mut self, segment: crate::ui::statusbar::StatusSegment) {
        let fold_action = if segment == crate::ui::statusbar::StatusSegment::Folds
            && !self.fold_regions.is_empty()
            && self.folded.len() == self.fold_regions.len()
        {
            "unfold_all"
        } else {
            "fold_all"
        };
        let action = if segment == crate::ui::statusbar::StatusSegment::Folds {
            fold_action
        } else if let Some(action) = segment.action_id() {
            action
        } else {
            return;
        };

        match action {
            "compare_against" => {
                self.revision_picker = Some(crate::search::RevisionPicker::new_for_tab(
                    &self.root,
                    crate::search::RevisionTab::Branches,
                ));
            }
            "worktree_picker" => {
                self.worktree_picker = Some(crate::search::WorktreePicker::new(&self.root));
            }
            "goto_line" => {
                self.focus = Focus::Content;
                self.goto_line = Some(crate::search::GotoLineState::new());
            }
            "theme_picker" => {
                self.context_menu = None;
                self.theme_picker = Some(crate::search::ThemePicker::default());
            }
            "fold_all" => {
                if !self.fold_regions.is_empty() {
                    self.fold_all();
                    self.mark_content_scrolled();
                }
            }
            "unfold_all" => {
                if !self.fold_regions.is_empty() {
                    self.unfold_all();
                    self.mark_content_scrolled();
                }
            }
            "show_about" => self.show_about = !self.show_about,
            "copy_path" => {
                self.focus = Focus::Content;
                self.copy_path_to_clipboard(false);
            }
            _ => {}
        }
    }

    /// Returns whether a click is on the painted scrollbar or its adjacent pane border.
    fn content_scrollbar_hit(&self, column: u16, row: u16) -> bool {
        let ca = self.geo.content_area;
        let row_inside = row >= ca.y && row < ca.y.saturating_add(ca.height);
        let right_cell = ca.x.saturating_add(ca.width.saturating_sub(1));
        let adjacent_border = ca.x.saturating_add(ca.width);
        self.show_scrollbar
            && self.display_line_count() > ca.height as usize
            && row_inside
            && (column == right_cell || column == adjacent_border)
    }

    /// Maps a mouse row to a content scroll position, used for scrollbar dragging.
    pub(super) fn set_scroll_from_mouse_y(&mut self, row: u16) {
        let total = self.display_line_count();
        let inner_h = self.geo.content_area.height as usize;
        if total <= inner_h || inner_h == 0 {
            return;
        }
        let thumb_size = (inner_h * inner_h / total).max(1);
        let scroll_range = total - inner_h;
        let track_range = inner_h.saturating_sub(thumb_size);
        if track_range == 0 {
            return;
        }
        let y = (row as usize).saturating_sub(self.geo.content_area.y as usize);
        let y = y.min(track_range);
        self.set_content_scroll(y * scroll_range / track_range);
    }
}
