//! Right-click context menu for `App`.
//!
//! `App::handle_mouse` routes a right-button press to
//! `open_tree_context_menu`/`open_content_context_menu`, which build a list of
//! actions relevant to the item under the cursor and stash it on `App`. This
//! module owns the menu's data model (`ContextMenuState`), the entry builders
//! (which actions appear for a tree file, a tree directory, or the content
//! pane), keyboard/mouse interaction while the menu is open, and the dispatch
//! of a chosen action onto existing `App` operations (open, open in editor /
//! default app, copy path, reveal in file manager, toggles, expand/collapse).
//! The popup itself is drawn by `ui::popups::context_menu`. Only one menu is
//! ever open: opening a new one replaces the current, and a further right-click
//! dismisses it without immediately re-opening on a different target.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};

use crate::config::static_keys;

use super::{rect_contains, App, Focus};

/// A selectable context-menu action. Each variant maps onto an existing `App`
/// operation; see `App::execute_context_action`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextActionId {
    /// Open the target: a file loads it, a directory toggles expansion.
    Open,
    /// Open the target file in `$EDITOR` (suspends the TUI).
    OpenInEditor,
    /// Open the target file with the OS default application.
    OpenExternal,
    /// Reveal the target in the OS file manager (its parent dir for files).
    RevealInFileManager,
    /// Copy the absolute path of the target.
    CopyPath,
    /// Copy the path of the target relative to the viewer root.
    CopyRelativePath,
    /// Copy the active text selection (content pane).
    CopySelection,
    /// Copy the line under the content cursor.
    CopyLine,
    /// Copy the whole open file (content pane).
    CopyFile,
    /// Toggle word wrap for the content pane.
    ToggleWordWrap,
    /// Toggle raw vs rendered markdown (content pane).
    ToggleRawMarkdown,
    /// Select the open file in the tree (content pane).
    RevealInTree,
    /// Expand a specific directory node.
    ExpandDir,
    /// Collapse a specific directory node.
    CollapseDir,
    /// Expand every directory in the tree.
    ExpandAll,
    /// Collapse every directory in the tree.
    CollapseAll,
}

/// One row of the context menu: a selectable action or a visual separator.
#[derive(Debug, Clone)]
pub enum ContextMenuEntry {
    /// A runnable action and the label drawn for it. Toggle labels embed the
    /// current state (e.g. "Word wrap: on") at build time.
    Action { id: ContextActionId, label: String },
    /// A non-selectable divider row between action groups.
    Separator,
}

/// What the context menu was opened on, so activating an entry can act on the
/// right-clicked row even if the tree has rebuilt (and re-indexed) since.
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    /// A specific tree node, captured by path and by index at open time.
    Tree { path: PathBuf, index: usize },
    /// The content pane; actions act on the currently open file.
    Content,
}

/// State for the open context menu.
#[derive(Debug, Clone)]
pub struct ContextMenuState {
    pub entries: Vec<ContextMenuEntry>,
    /// Index of the highlighted row; always points at an [`ContextMenuEntry::Action`].
    pub selected: usize,
    /// Screen position of the right-click that opened the menu.
    pub anchor: (u16, u16),
    pub target: ContextMenuTarget,
}

impl ContextMenuState {
    /// Length of the longest action label, used to size the popup.
    pub fn max_label_len(&self) -> usize {
        self.entries
            .iter()
            .filter_map(|e| match e {
                ContextMenuEntry::Action { label, .. } => Some(label.chars().count()),
                ContextMenuEntry::Separator => None,
            })
            .max()
            .unwrap_or(0)
    }
}

/// Returns the action id of an entry when it is a selectable action row.
fn entry_action_id(entry: &ContextMenuEntry) -> Option<ContextActionId> {
    match entry {
        ContextMenuEntry::Action { id, .. } => Some(*id),
        ContextMenuEntry::Separator => None,
    }
}

/// Builds the menu rows for a tree node. Directories get a collapse/expand
/// item for their own expansion state; files get editor/external-open items.
fn tree_entries(is_dir: bool, expanded: bool) -> Vec<ContextMenuEntry> {
    let mut entries = vec![ContextMenuEntry::Action {
        id: ContextActionId::Open,
        label: "Open".to_string(),
    }];
    if !is_dir {
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::OpenInEditor,
            label: "Open in editor".to_string(),
        });
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::OpenExternal,
            label: "Open with default app".to_string(),
        });
    }
    entries.push(ContextMenuEntry::Separator);
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::CopyPath,
        label: "Copy absolute path".to_string(),
    });
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::CopyRelativePath,
        label: "Copy relative path".to_string(),
    });
    entries.push(ContextMenuEntry::Separator);
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::RevealInFileManager,
        label: "Reveal in file manager".to_string(),
    });
    if is_dir {
        entries.push(ContextMenuEntry::Action {
            id: if expanded {
                ContextActionId::CollapseDir
            } else {
                ContextActionId::ExpandDir
            },
            label: if expanded {
                "Collapse".to_string()
            } else {
                "Expand".to_string()
            },
        });
    }
    entries.push(ContextMenuEntry::Separator);
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::ExpandAll,
        label: "Expand all".to_string(),
    });
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::CollapseAll,
        label: "Collapse all".to_string(),
    });
    entries
}

/// Builds the menu rows for the content pane. Excludes actions that are not
/// applicable right now: "Copy selection" needs a live selection, and the
/// markdown raw/rendered toggle needs the markdown plugin on a markdown file.
fn content_entries(app: &App) -> Vec<ContextMenuEntry> {
    let mut entries = Vec::new();
    let has_selection = app.selection.as_ref().is_some_and(|s| !s.is_empty());
    if has_selection {
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::CopySelection,
            label: "Copy selection".to_string(),
        });
    }
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::CopyLine,
        label: "Copy line".to_string(),
    });
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::CopyFile,
        label: "Copy file".to_string(),
    });
    entries.push(ContextMenuEntry::Separator);
    entries.push(ContextMenuEntry::Action {
        id: ContextActionId::ToggleWordWrap,
        label: if app.word_wrap {
            "Word wrap: on".to_string()
        } else {
            "Word wrap: off".to_string()
        },
    });
    let markdown_plugin_active = app.plugin_manager.is_plugin_active("markdown");
    let is_markdown = app
        .current_file
        .as_ref()
        .is_some_and(|p| crate::file::is_markdown_path(p));
    if markdown_plugin_active && (is_markdown || app.show_raw_markdown) {
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::ToggleRawMarkdown,
            label: if app.show_raw_markdown {
                "Raw markdown: on".to_string()
            } else {
                "Rendered markdown: on".to_string()
            },
        });
    }
    if app.current_file.is_some() {
        entries.push(ContextMenuEntry::Separator);
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::RevealInTree,
            label: "Reveal in tree".to_string(),
        });
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::OpenInEditor,
            label: "Open in editor".to_string(),
        });
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::OpenExternal,
            label: "Open with default app".to_string(),
        });
        entries.push(ContextMenuEntry::Separator);
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::CopyPath,
            label: "Copy absolute path".to_string(),
        });
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::CopyRelativePath,
            label: "Copy relative path".to_string(),
        });
        entries.push(ContextMenuEntry::Action {
            id: ContextActionId::RevealInFileManager,
            label: "Reveal in file manager".to_string(),
        });
    }
    entries
}

impl App {
    /// Opens the context menu over the tree row at `index`, selecting that row
    /// so shared selection-based operations act on the right-clicked node.
    pub(super) fn open_tree_context_menu(&mut self, index: usize, anchor: (u16, u16)) {
        let Some(node) = self.nodes.get(index) else {
            return;
        };
        self.tree_selected = index;
        self.focus = Focus::Tree;
        self.last_click = None;
        self.context_menu = Some(ContextMenuState {
            entries: tree_entries(node.is_dir, self.expanded.contains(&node.path)),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Tree {
                path: node.path.clone(),
                index,
            },
        });
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::ActionInvoked {
                action: "context_menu_open",
                source: crate::telemetry::ActionSource::Mouse,
            });
    }

    /// Opens the context menu over the content pane, targeting the open file.
    pub(super) fn open_content_context_menu(&mut self, anchor: (u16, u16)) {
        self.focus = Focus::Content;
        self.last_click = None;
        self.context_menu = Some(ContextMenuState {
            entries: content_entries(self),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Content,
        });
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::ActionInvoked {
                action: "context_menu_open",
                source: crate::telemetry::ActionSource::Mouse,
            });
    }

    /// Closes the context menu if it is open.
    pub(crate) fn close_context_menu(&mut self) {
        self.context_menu = None;
    }

    /// The path the open menu's target refers to, if any.
    fn context_target_path(&self) -> Option<&Path> {
        match &self.context_menu.as_ref()?.target {
            ContextMenuTarget::Tree { path, .. } => Some(path),
            ContextMenuTarget::Content => self.current_file.as_deref(),
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
            ContextMenuTarget::Content => false,
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
        let n = menu.entries.len();
        if n == 0 {
            return;
        }
        let mut next = (menu.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        while let Some(ContextMenuEntry::Separator) = menu.entries.get(next) {
            if delta > 0 {
                next = next.saturating_add(1);
            } else {
                next = next.saturating_sub(1);
            }
        }
        if next < n
            && matches!(
                menu.entries.get(next),
                Some(ContextMenuEntry::Action { .. })
            )
        {
            menu.selected = next;
        }
    }

    /// Handles keyboard input while the context menu is open. Esc closes, the
    /// arrow/vim navigation moves between actions (skipping separators), and
    /// Enter runs the highlighted action.
    pub(super) fn handle_context_menu_key(&mut self, key: KeyEvent) {
        if self.context_menu.is_none() {
            return;
        }
        if static_keys::is_close(&key) {
            self.close_context_menu();
            return;
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_context_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_context_selection(1),
            KeyCode::Enter => {
                let id = self
                    .context_menu
                    .as_ref()
                    .and_then(|m| m.entries.get(m.selected))
                    .and_then(entry_action_id);
                if let Some(id) = id {
                    self.execute_context_action(id);
                }
            }
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
    pub(super) fn handle_context_menu_mouse(&mut self, ev: MouseEvent) {
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
                if let Some(id) = menu.entries.get(rel).and_then(entry_action_id) {
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

    /// Runs a chosen context-menu action against the menu's target and closes
    /// the menu. Same behaviour as the equivalent keybindings; user-visible
    /// failures surface as status messages rather than being swallowed.
    pub(crate) fn execute_context_action(&mut self, id: ContextActionId) {
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::ActionInvoked {
                action: "context_menu_action",
                source: crate::telemetry::ActionSource::Mouse,
            });
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
        }
        self.close_context_menu();
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
#[path = "context_menu_test.rs"]
mod tests;
