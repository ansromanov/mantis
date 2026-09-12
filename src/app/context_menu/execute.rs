//! Execution of context-menu actions against their captured targets.
//!
//! This module resolves target paths, restores tree selection where existing
//! selection-based operations require it, and routes menu commands through
//! the application's established helpers. Errors for expected user actions
//! are surfaced through status messages by those helpers. Menu construction
//! and navigation are kept in the sibling context-menu modules.
//! Commands reuse existing application operations so keyboard and menu behavior agree.
//! Captured paths and row metadata keep actions aimed at the original click target.
//!

use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::app::{
    rect_contains, App, ContextActionId, ContextMenuEntry, ContextMenuTarget, Focus, TabAction,
};
use crate::config::static_keys;

use super::entries::{entry_action_id, markdown_code_fence};

impl App {
    /// The path the open menu's target refers to, if any.
    fn context_target_path(&self) -> Option<&Path> {
        match &self.context_menu.as_ref()?.target {
            ContextMenuTarget::Tree { path, .. } => Some(path),
            ContextMenuTarget::Content => self.current_file.as_deref(),
            ContextMenuTarget::Tab { root, .. } => Some(root),
            ContextMenuTarget::Breadcrumb { path } => Some(path),
            ContextMenuTarget::Blame { .. } | ContextMenuTarget::DiffHunk { .. } => {
                self.current_file.as_deref()
            }
            ContextMenuTarget::Statusbar => None,
        }
    }

    /// Points `tree_selected` at the menu's tree target (by path, falling back
    /// to the captured index), so shared selection-based operations act on the
    /// right-clicked node. Returns `false` when the menu targets the content
    /// pane or no tree target is recorded.
    fn select_context_tree_target(&mut self) -> bool {
        let Some(menu) = &self.context_menu else {
            return false;
        };
        match &menu.target {
            ContextMenuTarget::Content
            | ContextMenuTarget::Tab { .. }
            | ContextMenuTarget::Breadcrumb { .. }
            | ContextMenuTarget::Blame { .. }
            | ContextMenuTarget::DiffHunk { .. }
            | ContextMenuTarget::Statusbar => false,
            ContextMenuTarget::Tree { path, index } => {
                let i = self
                    .nodes
                    .iter()
                    .position(|n| n.path == *path)
                    .unwrap_or(*index);
                let i = i.min(self.nodes.len().saturating_sub(1));
                self.tree_selected = i;
                true
            }
        }
    }

    /// Moves the highlighted action by `delta` rows, skipping separators and
    /// clamping at both ends of the menu.
    fn move_context_selection(&mut self, delta: isize) {
        let Some(menu) = self.context_menu.as_mut() else {
            return;
        };
        let n = menu.visible_entries().len();
        if n == 0 {
            return;
        }
        let mut next = (menu.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        while matches!(
            menu.visible_entries().get(next),
            Some(ContextMenuEntry::Separator)
        ) {
            if delta > 0 {
                next = next.saturating_add(1);
            } else {
                next = next.saturating_sub(1);
            }
        }
        if next < n
            && matches!(
                menu.visible_entries().get(next),
                Some(ContextMenuEntry::Action { .. } | ContextMenuEntry::Submenu { .. })
            )
        {
            menu.set_visible_selected(next);
        }
    }

    /// Handles keyboard input while the context menu is open. Esc closes, the
    /// arrow/vim navigation moves between actions (skipping separators), and
    /// Enter runs the highlighted action.
    pub(crate) fn handle_context_menu_key(&mut self, key: KeyEvent) {
        if self.context_menu.is_none() {
            return;
        }
        if static_keys::is_close(&key) {
            if let Some(menu) = self.context_menu.as_mut() {
                let at_root = menu.submenu_stack.is_empty();
                if !at_root {
                    menu.submenu_stack.pop();
                } else {
                    self.close_context_menu();
                }
            }
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_context_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_context_selection(1),
            KeyCode::Left => {
                if let Some(menu) = self.context_menu.as_mut() {
                    menu.submenu_stack.pop();
                }
            }
            KeyCode::Right => self.open_selected_context_submenu(),
            KeyCode::Enter => self.activate_context_selection(),
            _ => {
                if static_keys::is_page_up(&key) {
                    self.move_context_selection(-10);
                } else if static_keys::is_page_down(&key) {
                    self.move_context_selection(10);
                }
            }
        }
    }

    /// Handles mouse input while the context menu is open: a left click on an
    /// action row runs it, a left click anywhere else dismisses the menu, and
    /// the scroll wheel navigates.
    pub(crate) fn handle_context_menu_mouse(&mut self, ev: MouseEvent) {
        match ev.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let Some(menu) = self.context_menu.as_ref() else {
                    return;
                };
                if !rect_contains(self.context_menu_area, ev.column, ev.row) {
                    self.close_context_menu();
                    return;
                }
                // The popup includes its border; action rows start one row in.
                let inner_left = self.context_menu_area.x + 1;
                let inner_right = self.context_menu_area.x + self.context_menu_area.width - 1;
                if ev.column < inner_left || ev.column >= inner_right {
                    return;
                }
                let inner_top = self.context_menu_area.y + 1;
                if ev.row < inner_top {
                    return;
                }
                let rel = (ev.row - inner_top) as usize;
                if let Some(ContextMenuEntry::Submenu { entries, .. }) =
                    menu.visible_entries().get(rel)
                {
                    let entries = entries.clone();
                    if let Some(menu) = self.context_menu.as_mut() {
                        menu.set_visible_selected(rel);
                        menu.push_submenu(entries);
                    }
                    return;
                }
                if let Some(id) = menu.visible_entries().get(rel).and_then(entry_action_id) {
                    self.execute_context_action(id);
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                // Right-click while the menu is open dismisses it; the next
                // right-click opens a fresh menu on the new target.
                self.close_context_menu();
            }
            MouseEventKind::ScrollDown => self.move_context_selection(1),
            MouseEventKind::ScrollUp => self.move_context_selection(-1),
            _ => {}
        }
    }

    fn activate_context_selection(&mut self) {
        let selected_entry = self
            .context_menu
            .as_ref()
            .and_then(|menu| menu.visible_entries().get(menu.visible_selected()).cloned());
        match selected_entry {
            Some(ContextMenuEntry::Action { id, .. }) => self.execute_context_action(id),
            Some(ContextMenuEntry::Submenu { entries, .. }) => {
                if let Some(menu) = self.context_menu.as_mut() {
                    menu.push_submenu(entries);
                }
            }
            _ => {}
        }
    }

    fn open_selected_context_submenu(&mut self) {
        let entries = self.context_menu.as_ref().and_then(|menu| {
            match menu.visible_entries().get(menu.visible_selected()) {
                Some(ContextMenuEntry::Submenu { entries, .. }) => Some(entries.clone()),
                _ => None,
            }
        });
        if let Some(entries) = entries {
            if let Some(menu) = self.context_menu.as_mut() {
                menu.push_submenu(entries);
            }
        }
    }

    /// Runs a chosen context-menu action against the menu's target and closes
    /// the menu. Same behaviour as the equivalent keybindings; user-visible
    /// failures surface as status messages rather than being swallowed.
    pub(crate) fn execute_context_action(&mut self, id: ContextActionId) {
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::ActionInvoked {
                action: "context_menu_action",
                source: crate::telemetry::ActionSource::Mouse,
            });
        if matches!(
            id,
            ContextActionId::FileHistory
                | ContextActionId::ToggleBlame
                | ContextActionId::DiffVsHead
        ) {
            if let Some(ContextMenuTarget::Tree { path, .. }) =
                self.context_menu.as_ref().map(|m| &m.target)
            {
                let path = path.clone();
                if path.is_file() {
                    self.open_file(&path);
                }
            }
        }
        match id {
            ContextActionId::Open => {
                if self.select_context_tree_target() {
                    self.activate_selected();
                }
            }
            ContextActionId::OpenInEditor => {
                let path = self.context_target_path().map(Path::to_path_buf);
                if let Some(p) = path {
                    if !self.current_file.as_deref().is_some_and(|cf| cf == p) {
                        self.open_file(&p);
                    }
                    self.open_in_editor();
                }
            }
            ContextActionId::OpenExternal => {
                if let Some(p) = self.context_target_path().map(Path::to_path_buf) {
                    self.open_external(&p);
                }
            }
            ContextActionId::RevealInFileManager => {
                if let Some(p) = self.context_target_path().map(Path::to_path_buf) {
                    self.open_in_file_manager(&p);
                }
            }
            ContextActionId::CopyPath => self.copy_context_target_path(false),
            ContextActionId::CopyRelativePath => self.copy_context_target_path(true),
            ContextActionId::CopySelection => {
                if let Some(sel) = &self.selection {
                    if !sel.is_empty() {
                        let text = self.selection_text();
                        if !text.is_empty() {
                            self.copy_to_clipboard(text, "selection");
                        }
                    }
                }
            }
            ContextActionId::CopyLine => self.copy_line_or_selection(),
            ContextActionId::CopyFile => self.copy_file_content(),
            ContextActionId::ToggleWordWrap => self.toggle_word_wrap(),
            ContextActionId::ToggleRawMarkdown => self.toggle_raw_markdown(),
            ContextActionId::RevealInTree => {
                let current = self.current_file.clone();
                if let Some(p) = current {
                    self.reveal_in_tree(&p);
                    self.focus = Focus::Tree;
                    self.scroll_tree_into_view();
                }
            }
            ContextActionId::ExpandDir | ContextActionId::CollapseDir => {
                if self.select_context_tree_target() {
                    if let Some(node) = self.nodes.get(self.tree_selected) {
                        if node.is_dir {
                            if id == ContextActionId::ExpandDir {
                                self.expanded.insert(node.path.clone());
                            } else {
                                self.expanded.remove(&node.path);
                            }
                            self.mark_session_dirty();
                            self.rebuild(true);
                            self.scroll_tree_into_view();
                        }
                    }
                }
            }
            ContextActionId::ExpandAll => self.expand_all(),
            ContextActionId::CollapseAll => self.collapse_all(),
            ContextActionId::OpenInNewTab => {
                if self.select_context_tree_target() {
                    if let Some(node) = self.nodes.get(self.tree_selected) {
                        let path = if node.is_dir {
                            node.path.clone()
                        } else {
                            node.path.parent().unwrap_or(&self.root).to_path_buf()
                        };
                        self.tab_action_request = Some(TabAction::Open(path));
                    }
                }
            }
            ContextActionId::SetAsRoot => {
                if self.select_context_tree_target() {
                    if let Some(path) = self.nodes.get(self.tree_selected).map(|n| n.path.clone()) {
                        if path.is_dir() {
                            self.set_root(&path);
                        }
                    }
                } else if let Some(path) = self.context_target_path().map(Path::to_path_buf) {
                    if path.is_dir() {
                        self.set_root(&path);
                    }
                }
            }
            ContextActionId::ToggleBookmark => {
                if let Some(path) = self.context_target_path().map(Path::to_path_buf) {
                    self.toggle_bookmark_path(path);
                }
            }
            ContextActionId::FindInFolder => {
                if let Some(path) = self.context_target_path().map(Path::to_path_buf) {
                    let root = if path.is_dir() {
                        path
                    } else {
                        path.parent().unwrap_or(&self.root).to_path_buf()
                    };
                    let mut search = crate::search::SearchState::new(
                        &root,
                        self.show_hidden,
                        self.ignore_gitignore,
                        self.config.search.context_lines,
                        None,
                    );
                    search.toggle_mode();
                    self.search = Some(search);
                    self.focus = Focus::Tree;
                }
            }
            ContextActionId::CollapseSiblings => {
                if self.select_context_tree_target() {
                    if let Some(path) = self.nodes.get(self.tree_selected).map(|n| n.path.clone()) {
                        if let Some(parent) = path.parent() {
                            self.expanded.retain(|p| p.parent() != Some(parent));
                            self.mark_session_dirty();
                            self.rebuild(false);
                            self.scroll_tree_into_view();
                        }
                    }
                }
            }
            ContextActionId::CopyFileName => {
                if let Some(path) = self.context_target_path().map(Path::to_path_buf) {
                    let name = path
                        .file_name()
                        .unwrap_or(path.as_os_str())
                        .to_string_lossy()
                        .to_string();
                    self.copy_to_clipboard(name, "file name");
                }
            }
            ContextActionId::CopyMarkdownBlock => self.copy_context_markdown_block(),
            ContextActionId::BlameLine => {
                if self.has_text_cursor() {
                    self.show_line_blame = true;
                }
            }
            ContextActionId::GotoLine => self.goto_line = Some(crate::search::GotoLineState::new()),
            ContextActionId::FoldAll => self.fold_all(),
            ContextActionId::UnfoldAll => self.unfold_all(),
            ContextActionId::OpenAtRevision => self.toggle_file_revision(),
            ContextActionId::FileHistory => self.open_file_history(),
            ContextActionId::ToggleBlame => self.show_blame = !self.show_blame,
            ContextActionId::DiffVsHead => {
                if let Some(path) = self.context_target_path().map(Path::to_path_buf) {
                    self.open_file(&path);
                    if !self.git_mode {
                        self.toggle_git_mode();
                    }
                }
            }
            ContextActionId::CloseOtherTabs => {
                if let Some(ContextMenuTarget::Tab { index, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.tab_action_request = Some(TabAction::CloseOthers(*index));
                }
            }
            ContextActionId::CloseTabsToRight => {
                if let Some(ContextMenuTarget::Tab { index, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.tab_action_request = Some(TabAction::CloseToRight(*index));
                }
            }
            ContextActionId::DuplicateTab => {
                if let Some(ContextMenuTarget::Tab { root, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.tab_action_request = Some(TabAction::Open(root.clone()));
                }
            }
            ContextActionId::RevealRoot => {
                if let Some(root) = self.context_target_path().map(Path::to_path_buf) {
                    self.open_in_file_manager(&root);
                }
            }
            ContextActionId::OpenBlameCommit => {
                if let Some(ContextMenuTarget::Blame { line, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.active_line = self.physical_to_display(*line);
                    self.open_blame_commit_at_active_line();
                }
            }
            ContextActionId::CopyBlameHash => {
                if let Some(ContextMenuTarget::Blame { hash, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.copy_to_clipboard(hash.clone(), "commit hash");
                }
            }
            ContextActionId::CopyBlameAuthor => {
                if let Some(ContextMenuTarget::Blame { author, .. }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    self.copy_to_clipboard(author.clone(), "author");
                }
            }
            ContextActionId::NextHunk => self.diff_next_hunk(),
            ContextActionId::PreviousHunk => self.diff_prev_hunk(),
            ContextActionId::CopyHunk => {
                if let Some(ContextMenuTarget::DiffHunk { header_row }) =
                    self.context_menu.as_ref().map(|m| &m.target)
                {
                    let end = self
                        .content
                        .iter()
                        .enumerate()
                        .skip(header_row.saturating_add(1))
                        .find_map(|(i, line)| line.starts_with("@@").then_some(i))
                        .unwrap_or(self.content.len());
                    let hunk = self
                        .content
                        .get(*header_row..end)
                        .map(|rows| rows.join("\n"))
                        .unwrap_or_default();
                    self.copy_to_clipboard(hunk, "diff hunk");
                }
            }
            ContextActionId::StatusRecentFiles => self.open_recent_files(),
            ContextActionId::StatusBookmarks => self.open_bookmarks(),
            ContextActionId::StatusThemePicker => {
                self.theme_picker = Some(crate::search::ThemePicker::default())
            }
            ContextActionId::StatusWorktreePicker => {
                self.worktree_picker = Some(crate::search::WorktreePicker::new(&self.root));
            }
            ContextActionId::StatusRepoLog => self.open_repo_log(),
        }
        self.close_context_menu();
    }

    fn copy_context_markdown_block(&mut self) {
        let Some(path) = self.context_target_path().map(Path::to_path_buf) else {
            self.set_status("nothing selected");
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) => {
                self.set_status(format!("cannot read {}: {error}", path.display()));
                return;
            }
        };
        let relative = path.strip_prefix(&self.root).unwrap_or(&path).display();
        let line = if matches!(
            self.context_menu.as_ref().map(|m| &m.target),
            Some(ContextMenuTarget::Content)
        ) {
            self.active_line.saturating_add(1)
        } else {
            1
        };
        let language = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let fence = markdown_code_fence(&text);
        let block = format!("{relative}:{line}\n\n{fence}{language}\n{text}\n{fence}");
        self.copy_to_clipboard(block, "markdown block");
    }

    /// Copies the absolute (or viewer-root-relative) path of the menu's target.
    fn copy_context_target_path(&mut self, relative: bool) {
        let Some(path) = self.context_target_path().map(Path::to_path_buf) else {
            self.set_status("nothing selected");
            return;
        };
        let text = if relative {
            path.strip_prefix(&self.root)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string())
        } else {
            path.display().to_string()
        };
        self.copy_to_clipboard(text, if relative { "relative path" } else { "path" });
    }
}

#[cfg(test)]
#[path = "execute_test.rs"]
mod tests;
