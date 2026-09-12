//! Context-menu row builders for each supported application target.
//!
//! Builders keep action availability and labels in one place so menus only
//! show operations that make sense for the current file, tree node, or status
//! segment. The module also owns the markdown fence helper used when copying
//! source as a fenced block. Rows are consumed by the parent input module and
//! tested here as well as through end-to-end menu-opening tests.
//! Git actions are filtered through the shared action applicability registry.
//! Labels and available operations therefore stay in sync with application state.
//!

use std::path::Path;

use crate::app::{App, ContextActionId, ContextMenuEntry};

/// Returns the action id of an entry when it is a selectable action row.
pub(super) fn entry_action_id(entry: &ContextMenuEntry) -> Option<ContextActionId> {
    match entry {
        ContextMenuEntry::Action { id, .. } => Some(*id),
        ContextMenuEntry::Separator | ContextMenuEntry::Submenu { .. } => None,
    }
}

/// Builds the menu rows for a tree node. Directories get a collapse/expand
/// item for their own expansion state; files get editor/external-open items.
fn action(id: ContextActionId, label: &str) -> ContextMenuEntry {
    ContextMenuEntry::Action {
        id,
        label: label.to_string(),
    }
}

fn action_applicable_to_path(app: &App, action_id: &str, path: &Path) -> bool {
    use crate::actions::Applicability;

    let applicability = crate::actions::ACTIONS
        .iter()
        .find(|spec| spec.id == action_id)
        .map(|spec| spec.applicability());
    match applicability {
        Some(Applicability::GitRepoAndFile | Applicability::GitRepoAndNoDiff) => {
            app.git_info.is_some()
                && path.is_file()
                && (applicability != Some(Applicability::GitRepoAndNoDiff) || !app.is_diff)
        }
        Some(Applicability::GitRepo) => app.git_info.is_some(),
        _ => app.check_applicability(action_id).is_ok(),
    }
}

pub(super) fn tree_entries(
    app: &App,
    path: &Path,
    is_dir: bool,
    expanded: bool,
) -> Vec<ContextMenuEntry> {
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
    if is_dir {
        entries.push(action(ContextActionId::OpenInNewTab, "Open in new tab"));
        entries.push(action(ContextActionId::SetAsRoot, "Set as tree root"));
        entries.push(action(ContextActionId::FindInFolder, "Find in this folder"));
        entries.push(action(
            ContextActionId::CollapseSiblings,
            "Collapse siblings",
        ));
    } else {
        entries.push(action(ContextActionId::ToggleBookmark, "Toggle bookmark"));
        entries.push(action(ContextActionId::CopyFileName, "Copy file name"));
        entries.push(action(
            ContextActionId::CopyMarkdownBlock,
            "Copy as markdown block",
        ));
    }
    let git_actions = [
        ("file_history", ContextActionId::FileHistory, "File history"),
        ("toggle_blame", ContextActionId::ToggleBlame, "Toggle blame"),
        (
            "git_mode_toggle",
            ContextActionId::DiffVsHead,
            "Diff vs HEAD",
        ),
    ];
    let git_entries: Vec<_> = git_actions
        .into_iter()
        .filter(|(id, _, _)| action_applicable_to_path(app, id, path))
        .map(|(_, id, label)| action(id, label))
        .collect();
    if !git_entries.is_empty() && !is_dir {
        entries.push(ContextMenuEntry::Submenu {
            label: "Git".to_string(),
            entries: git_entries,
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
pub(super) fn content_entries(app: &App) -> Vec<ContextMenuEntry> {
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
        entries.push(action(
            ContextActionId::CopyMarkdownBlock,
            "Copy as markdown block",
        ));
        if app.check_applicability("goto_line").is_ok() {
            entries.push(action(ContextActionId::GotoLine, "Go to line"));
        }
        if app.check_applicability("blame_line").is_ok() {
            entries.push(action(ContextActionId::BlameLine, "Blame this line"));
        }
        if app.check_applicability("fold_all").is_ok() {
            entries.push(action(ContextActionId::FoldAll, "Fold all"));
            entries.push(action(ContextActionId::UnfoldAll, "Unfold all"));
        }
        if app.check_applicability("toggle_file_revision").is_ok() {
            entries.push(action(ContextActionId::OpenAtRevision, "Open at revision"));
        }
        let git_entries: Vec<_> = [
            ("file_history", ContextActionId::FileHistory, "File history"),
            ("toggle_blame", ContextActionId::ToggleBlame, "Toggle blame"),
        ]
        .into_iter()
        .filter(|(id, _, _)| app.check_applicability(id).is_ok())
        .map(|(_, id, label)| action(id, label))
        .collect();
        if !git_entries.is_empty() {
            entries.push(ContextMenuEntry::Submenu {
                label: "Git".to_string(),
                entries: git_entries,
            });
        }
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

pub(super) fn tab_entries() -> Vec<ContextMenuEntry> {
    vec![
        action(ContextActionId::CloseOtherTabs, "Close other tabs"),
        action(ContextActionId::CloseTabsToRight, "Close tabs to the right"),
        action(ContextActionId::DuplicateTab, "Duplicate tab"),
        action(ContextActionId::RevealRoot, "Reveal root in file manager"),
    ]
}

pub(super) fn breadcrumb_entries() -> Vec<ContextMenuEntry> {
    vec![
        action(ContextActionId::SetAsRoot, "Set as tree root"),
        action(ContextActionId::CopyPath, "Copy absolute path"),
        action(ContextActionId::CopyRelativePath, "Copy relative path"),
        action(
            ContextActionId::RevealInFileManager,
            "Reveal in file manager",
        ),
    ]
}

pub(super) fn blame_entries() -> Vec<ContextMenuEntry> {
    vec![
        action(ContextActionId::OpenBlameCommit, "Open commit"),
        action(ContextActionId::CopyBlameHash, "Copy commit hash"),
        action(ContextActionId::CopyBlameAuthor, "Copy author"),
    ]
}

pub(super) fn hunk_entries() -> Vec<ContextMenuEntry> {
    vec![
        action(ContextActionId::PreviousHunk, "Previous hunk"),
        action(ContextActionId::NextHunk, "Next hunk"),
        action(ContextActionId::CopyHunk, "Copy hunk"),
    ]
}

pub(super) fn markdown_code_fence(text: &str) -> String {
    let mut max_run = 0usize;
    let mut run = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            max_run = max_run.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat(max_run.max(2) + 1)
}

pub(super) fn statusbar_entries(app: &App, segment: &str) -> Vec<ContextMenuEntry> {
    let label = segment.trim().to_ascii_lowercase();
    if label.starts_with("cursor ln") {
        return if app.check_applicability("goto_line").is_ok() {
            vec![action(ContextActionId::GotoLine, "Go to line")]
        } else {
            Vec::new()
        };
    }
    if app
        .git_info
        .as_ref()
        .is_some_and(|info| label.contains(&info.head.display().to_ascii_lowercase()))
    {
        return vec![
            action(ContextActionId::StatusRepoLog, "Open repository history"),
            action(
                ContextActionId::StatusWorktreePicker,
                "Open worktree picker",
            ),
        ];
    }
    if label.contains("worktree") {
        return vec![action(
            ContextActionId::StatusWorktreePicker,
            "Open worktree picker",
        )];
    }
    if label.contains("bookmark") {
        return vec![action(ContextActionId::StatusBookmarks, "Open bookmarks")];
    }
    if label.contains("theme") {
        return vec![action(
            ContextActionId::StatusThemePicker,
            "Open theme picker",
        )];
    }
    if label.contains("recent") {
        return vec![action(
            ContextActionId::StatusRecentFiles,
            "Open recent files",
        )];
    }
    vec![
        action(ContextActionId::StatusRecentFiles, "Open recent files"),
        action(ContextActionId::StatusBookmarks, "Open bookmarks"),
        action(ContextActionId::StatusThemePicker, "Open theme picker"),
    ]
}

#[cfg(test)]
#[path = "entries_test.rs"]
mod tests;
