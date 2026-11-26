//! Mouse handling for the picker, popup and bar overlays (recent files, bookmarks, history,
//! theme, command palette, search, bug report, help, filter bar, revision and worktree pickers).

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::content_pos::WHEEL_STEP;
use super::mouse_handlers::{handle_picker_mouse, PickerMouseAction};
use super::{rect_contains, App};

impl App {
    /// Handles mouse events on the recent-files overlay: click to open,
    /// outside click to close, scroll to navigate.
    pub(super) fn handle_recent_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.geo.recent_area, ev.column, ev.row) {
                    let row = (ev.row - self.geo.recent_area.y) as usize;
                    let index = self.geo.recent_offset + row;
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

    /// Handles mouse events on the bookmarks overlay.
    pub(super) fn handle_bookmarks_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if rect_contains(self.geo.recent_area, ev.column, ev.row) {
                    let row = (ev.row - self.geo.recent_area.y) as usize;
                    let index = self.geo.recent_offset + row;
                    let in_range = self
                        .bookmarks
                        .as_ref()
                        .is_some_and(|picker| index < picker.results_len());
                    if in_range {
                        if let Some(picker) = &mut self.bookmarks {
                            picker.selected = index;
                        }
                        self.activate_bookmark_selection();
                    }
                } else {
                    self.bookmarks = None;
                }
            }
            MouseEventKind::ScrollUp => {
                if let Some(picker) = &mut self.bookmarks {
                    picker.selected = picker.selected.saturating_sub(1);
                }
            }
            MouseEventKind::ScrollDown => {
                if let Some(picker) = &mut self.bookmarks {
                    if picker.selected + 1 < picker.results_len() {
                        picker.selected += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// Handles mouse events on the git-history overlay: click to select,
    /// double-click to open the diff, scroll to navigate.
    /// Close on click outside the popup area.
    pub(super) fn handle_history_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.history_area,
            self.geo.history_offset,
            &mut self.history,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.show_selected_revision();
        }
    }

    /// Handles mouse events on the repo-wide commit log overlay: click to
    /// select, double-click to enter compare mode, scroll to navigate.
    /// Close on click outside the popup area.
    pub(super) fn handle_repo_log_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.repo_log_area,
            self.geo.repo_log_offset,
            &mut self.repo_log,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            self.show_repo_log_compare();
        }
    }

    /// Handles mouse events on the theme picker overlay: click to select,
    /// double-click to apply, scroll to navigate.
    /// Close on click outside the popup area.
    pub(super) fn handle_theme_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.theme_area,
            self.geo.theme_offset,
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
    pub(super) fn handle_command_palette_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.command_palette_area,
            self.geo.command_palette_offset,
            &mut self.command_palette,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            let route = self
                .command_palette
                .as_ref()
                .map(|p| p.route)
                .unwrap_or(crate::command_palette::PaletteRoute::Commands);
            match route {
                crate::command_palette::PaletteRoute::Commands => {
                    self.dispatch_command();
                }
                crate::command_palette::PaletteRoute::Files => self.dispatch_palette_file(),
                crate::command_palette::PaletteRoute::Content => self.dispatch_palette_content(),
                crate::command_palette::PaletteRoute::GotoLine => {}
                crate::command_palette::PaletteRoute::Symbols => self.dispatch_palette_symbol(),
                crate::command_palette::PaletteRoute::Diagnostics => {
                    self.dispatch_palette_diagnostic()
                }
            }
        }
    }

    /// Handles mouse events for the bug report modal.
    pub(super) fn handle_bug_report_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::ScrollDown => {
                if rect_contains(self.geo.bug_report_preview_area, ev.column, ev.row) {
                    if let Some(ref mut state) = self.bug_report {
                        let body_text = state.text.join("\n");
                        let report_md = if body_text.trim().is_empty() {
                            state.diagnostics_markdown.clone()
                        } else {
                            format!(
                                "## {}\n\n## bug report body\n\n{}\n\n{}",
                                state.resolved_title(),
                                body_text,
                                state.diagnostics_markdown
                            )
                        };
                        let total_lines = report_md.lines().count();
                        let preview_height = self.geo.bug_report_preview_area.height as usize;
                        let max_scroll = total_lines.saturating_sub(preview_height);
                        state.preview_scroll.scroll_down(WHEEL_STEP, max_scroll);
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                if rect_contains(self.geo.bug_report_preview_area, ev.column, ev.row) {
                    if let Some(ref mut state) = self.bug_report {
                        state.preview_scroll.scroll_up(WHEEL_STEP);
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Left)
                if !rect_contains(self.geo.bug_report_area, ev.column, ev.row) =>
            {
                self.bug_report = None;
            }
            _ => {}
        }
    }

    pub(super) fn handle_welcome_mouse(&mut self, ev: MouseEvent) {
        if let MouseEventKind::Down(MouseButton::Left) = ev.kind {
            if !rect_contains(self.geo.welcome_area, ev.column, ev.row) {
                self.show_welcome = false;
                crate::session::mark_welcome_shown();
            }
        }
    }

    pub(super) fn handle_help_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::ScrollDown => {
                self.help_scroll.scroll_down(WHEEL_STEP, usize::MAX);
            }
            MouseEventKind::ScrollUp => {
                self.help_scroll.scroll_up(WHEEL_STEP);
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if !rect_contains(self.geo.help_area, ev.column, ev.row) {
                    self.show_help = false;
                    self.help_scroll.scroll = 0;
                    self.help_tab = 0;
                    return;
                }
                if ev.row == self.geo.help_area.y + 1 && ev.column > self.geo.help_area.x {
                    let available_width = self.geo.help_area.width.saturating_sub(2);
                    let offset =
                        crate::ui::popups::help_tab_scroll_offset(self.help_tab, available_width);
                    let content_col = (ev.column - (self.geo.help_area.x + 1)) + offset;
                    let ranges = crate::ui::popups::help_tab_ranges(0);
                    for (i, (start, end)) in ranges.iter().enumerate() {
                        if content_col >= *start && content_col < *end {
                            if self.help_tab != i {
                                self.help_tab = i;
                                self.help_scroll.scroll = 0;
                            }
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub(super) fn handle_filter_bar_mouse(&mut self, ev: MouseEvent) {
        if let MouseEventKind::Down(MouseButton::Left) = ev.kind {
            let ca = self.geo.content_area;
            let bar_y = ca.y + ca.height.saturating_sub(2);
            let on_bar = ev.row == bar_y && ev.column >= ca.x && ev.column < ca.x + ca.width;
            if !on_bar {
                if let Some(s) = &self.filter_bar {
                    if s.query.is_empty() {
                        self.filter_query = None;
                    } else {
                        self.filter_query = Some(s.query.clone());
                    }
                }
                self.filter_bar = None;
                self.rebuild_filter_display_map();
                self.clamp_content_scroll();
            }
        }
    }

    pub(super) fn handle_revision_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.revision_picker_area,
            self.geo.revision_picker_offset,
            &mut self.revision_picker,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            let rev = self.revision_picker.as_ref().and_then(|p| {
                if p.results_len() > 0 && p.selected < p.results_len() {
                    p.selected_rev().map(|r| r.to_string())
                } else if !p.query.is_empty() {
                    Some(p.query.clone())
                } else {
                    None
                }
            });
            self.revision_picker = None;
            if let Some(rev) = rev {
                self.enter_compare_mode(rev);
            }
        }
    }

    pub(super) fn handle_worktree_mouse(&mut self, ev: MouseEvent) {
        if handle_picker_mouse(
            ev,
            self.geo.worktree_picker_area,
            self.geo.worktree_picker_offset,
            &mut self.worktree_picker,
            &mut self.last_click,
        ) == PickerMouseAction::Activate
        {
            let path = self
                .worktree_picker
                .as_ref()
                .and_then(|p| p.selected_path());
            self.worktree_picker = None;
            if let Some(path) = path {
                self.set_root(&path);
            }
        }
    }
}
