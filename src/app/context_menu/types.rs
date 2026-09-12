//! Shared context-menu data types and navigation state.
//!
//! `ContextActionId` identifies commands independently from their displayed
//! labels. `ContextMenuEntry` describes action, separator, and submenu rows.
//! `ContextMenuTarget` captures the object selected when the menu opens, while
//! `ContextMenuState` tracks the root list, selection, anchor, and nested levels.
//! These types are consumed by input handling, action execution, and rendering.
//! The model remains independent from ratatui widgets and rendering geometry.
//!

use std::path::PathBuf;

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
    /// Open the selected tree directory in a new workspace tab.
    OpenInNewTab,
    /// Make the selected tree directory the active root.
    SetAsRoot,
    /// Toggle a bookmark for the selected tree file.
    ToggleBookmark,
    /// Search file contents within the selected tree directory.
    FindInFolder,
    /// Collapse expanded sibling directories.
    CollapseSiblings,
    /// Copy the target file's basename.
    CopyFileName,
    /// Copy the target file as a fenced markdown block with path and line.
    CopyMarkdownBlock,
    /// Open blame for the active content line.
    BlameLine,
    /// Open the go-to-line prompt.
    GotoLine,
    /// Fold every region in the active content.
    FoldAll,
    /// Unfold every region in the active content.
    UnfoldAll,
    /// Toggle the content between the current diff and revision snapshot.
    OpenAtRevision,
    /// Open the active file's git history.
    FileHistory,
    /// Toggle full-file blame annotations.
    ToggleBlame,
    /// Toggle git mode to show the target's diff against HEAD.
    DiffVsHead,
    /// Close every tab except the context-menu target.
    CloseOtherTabs,
    /// Close tabs to the right of the context-menu target.
    CloseTabsToRight,
    /// Duplicate the context-menu target tab.
    DuplicateTab,
    /// Reveal the tab's project root in the file manager.
    RevealRoot,
    /// Open the commit attached to a blame row.
    OpenBlameCommit,
    /// Copy the blame commit hash.
    CopyBlameHash,
    /// Copy the blame author name.
    CopyBlameAuthor,
    /// Jump to the next diff hunk.
    NextHunk,
    /// Jump to the previous diff hunk.
    PreviousHunk,
    /// Copy the active diff hunk.
    CopyHunk,
    /// Open the recent-files picker.
    StatusRecentFiles,
    /// Open the theme picker.
    StatusThemePicker,
    /// Open the bookmarks picker.
    StatusBookmarks,
    /// Open the worktree picker.
    StatusWorktreePicker,
    /// Open the repository commit log.
    StatusRepoLog,
}

/// One row of the context menu: a selectable action or a visual separator.
#[derive(Debug, Clone)]
pub enum ContextMenuEntry {
    /// A runnable action and the label drawn for it. Toggle labels embed the
    /// current state (e.g. "Word wrap: on") at build time.
    Action { id: ContextActionId, label: String },
    /// A non-selectable divider row between action groups.
    Separator,
    /// A selectable row that opens a nested list of entries.
    Submenu {
        label: String,
        entries: Vec<ContextMenuEntry>,
    },
}

/// A nested menu level's rows and current selection.
#[derive(Debug, Clone)]
pub struct ContextMenuLevel {
    pub entries: Vec<ContextMenuEntry>,
    pub selected: usize,
}

/// What the context menu was opened on, so activating an entry can act on the
/// right-clicked row even if the tree has rebuilt (and re-indexed) since.
#[derive(Debug, Clone)]
pub enum ContextMenuTarget {
    /// A specific tree node, captured by path and by index at open time.
    Tree { path: PathBuf, index: usize },
    /// The content pane; actions act on the currently open file.
    Content,
    /// A workspace tab, captured by project root and tab index.
    Tab { root: PathBuf, index: usize },
    /// A breadcrumb segment.
    Breadcrumb { path: PathBuf },
    /// A blame annotation, captured at the clicked line.
    Blame {
        line: usize,
        hash: String,
        author: String,
    },
    /// A diff hunk, captured by its zero-based header row.
    DiffHunk { header_row: usize },
    /// A visible status-bar segment.
    Statusbar,
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
    /// Open submenu levels, with the deepest level last.
    pub submenu_stack: Vec<ContextMenuLevel>,
}

impl ContextMenuState {
    /// Rows in the currently visible menu level.
    pub fn visible_entries(&self) -> &[ContextMenuEntry] {
        self.submenu_stack
            .last()
            .map_or(self.entries.as_slice(), |level| level.entries.as_slice())
    }

    /// Selected row in the currently visible menu level.
    pub fn visible_selected(&self) -> usize {
        self.submenu_stack
            .last()
            .map_or(self.selected, |level| level.selected)
    }

    pub(super) fn set_visible_selected(&mut self, selected: usize) {
        if let Some(level) = self.submenu_stack.last_mut() {
            level.selected = selected;
        } else {
            self.selected = selected;
        }
    }

    pub(super) fn push_submenu(&mut self, entries: Vec<ContextMenuEntry>) {
        self.submenu_stack.push(ContextMenuLevel {
            entries,
            selected: 0,
        });
    }
}

impl ContextMenuState {
    /// Length of the longest action label, used to size the popup.
    pub fn max_label_len(&self) -> usize {
        self.entries
            .iter()
            .filter_map(|e| match e {
                ContextMenuEntry::Action { label, .. } => Some(label.chars().count()),
                ContextMenuEntry::Separator => None,
                ContextMenuEntry::Submenu { label, .. } => Some(label.chars().count() + 3),
            })
            .max()
            .unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "types_test.rs"]
mod tests;
