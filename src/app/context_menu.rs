//! Context-menu input and target selection for the application.
//!
//! This module opens menus at the tree, content, tab, breadcrumb, blame, diff,
//! and status-bar targets, then handles keyboard and mouse navigation through
//! their rows. Target-specific row construction lives in `entries`; action
//! execution and path resolution live in `execute`; shared menu state and
//! entry types live in `types`. Rendering is implemented by
//! `ui::popups::context_menu`.
//! This keeps target capture separate from rendering details.
//!

use std::path::PathBuf;

use super::{App, Focus};
use entries::{
    blame_entries, breadcrumb_entries, content_entries, hunk_entries, statusbar_entries,
    tab_entries, tree_entries,
};
pub use types::{ContextActionId, ContextMenuEntry, ContextMenuState, ContextMenuTarget};

mod entries;
mod execute;
mod types;

impl App {
    /// Opens a context menu for the clicked status-bar segment.
    pub(super) fn open_statusbar_context_menu(&mut self, segment: String, anchor: (u16, u16)) {
        self.context_menu = Some(ContextMenuState {
            entries: statusbar_entries(self, &segment),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Statusbar,
            submenu_stack: Vec::new(),
        });
    }

    /// Opens a context menu over a workspace tab.
    pub(crate) fn open_tab_context_menu(
        &mut self,
        root: PathBuf,
        index: usize,
        anchor: (u16, u16),
    ) {
        self.last_click = None;
        self.context_menu = Some(ContextMenuState {
            entries: tab_entries(),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Tab { root, index },
            submenu_stack: Vec::new(),
        });
    }

    /// Opens a context menu over a breadcrumb segment.
    pub(super) fn open_breadcrumb_context_menu(&mut self, path: PathBuf, anchor: (u16, u16)) {
        self.context_menu = Some(ContextMenuState {
            entries: breadcrumb_entries(),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Breadcrumb { path },
            submenu_stack: Vec::new(),
        });
    }

    /// Opens a context menu for a blame annotation line.
    pub(super) fn open_blame_context_menu(&mut self, line: usize, anchor: (u16, u16)) {
        let Some(path) = self.current_file.as_ref() else {
            return;
        };
        let Some(blame) = crate::git::file_blame(&self.root, path)
            .into_iter()
            .find(|bl| bl.line_no as usize == line + 1)
        else {
            return;
        };
        self.context_menu = Some(ContextMenuState {
            entries: blame_entries(),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Blame {
                line,
                hash: blame.commit_hash,
                author: blame.author,
            },
            submenu_stack: Vec::new(),
        });
    }

    /// Opens a context menu for a diff hunk header row.
    pub(super) fn open_hunk_context_menu(&mut self, header_row: usize, anchor: (u16, u16)) {
        self.context_menu = Some(ContextMenuState {
            entries: hunk_entries(),
            selected: 0,
            anchor,
            target: ContextMenuTarget::DiffHunk { header_row },
            submenu_stack: Vec::new(),
        });
    }

    /// Maps a rendered diff row to its unified-text hunk header row.
    pub(super) fn diff_hunk_header_for_row(&self, display_row: usize) -> Option<usize> {
        if self.diff_side_by_side {
            let header = self
                .diff_rows
                .iter()
                .take(display_row.saturating_add(1))
                .rev()
                .find_map(|row| match row {
                    crate::diff::DiffRow::Header(header) => Some(header),
                    crate::diff::DiffRow::Split { .. } => None,
                })?;
            self.content.iter().position(|line| line == header)
        } else {
            self.content
                .get(display_row)
                .filter(|line| line.starts_with("@@"))
                .map(|_| display_row)
                .or_else(|| {
                    self.content
                        .iter()
                        .enumerate()
                        .take(display_row.saturating_add(1))
                        .rev()
                        .find_map(|(i, line)| line.starts_with("@@").then_some(i))
                })
        }
    }
    /// Opens a context menu at the focused tree row or active content line.
    pub(super) fn open_focused_context_menu(&mut self) {
        match self.focus {
            Focus::Tree => {
                let row = self.tree_selected.saturating_sub(self.tree_offset);
                let anchor = (
                    self.tree_area.x.saturating_add(2),
                    self.tree_area.y.saturating_add(row as u16),
                );
                self.open_tree_context_menu(self.tree_selected, anchor);
            }
            Focus::Content => {
                let row = self.active_line.saturating_sub(self.content_scroll);
                let anchor = (
                    self.content_area
                        .x
                        .saturating_add(self.line_prefix_width() as u16),
                    self.content_area.y.saturating_add(row as u16),
                );
                self.open_content_context_menu(anchor);
            }
        }
    }
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
            entries: tree_entries(
                self,
                &node.path,
                node.is_dir,
                self.expanded.contains(&node.path),
            ),
            selected: 0,
            anchor,
            target: ContextMenuTarget::Tree {
                path: node.path.clone(),
                index,
            },
            submenu_stack: Vec::new(),
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
            submenu_stack: Vec::new(),
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
}

#[cfg(test)]
#[path = "context_menu_test.rs"]
mod tests;
