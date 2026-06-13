use std::path::PathBuf;

use crate::git::GitStatus;
use crate::search::SearchMode;
use crate::tree::{build_visible, TreeNode};

use super::App;

impl App {
    /// Rebuilds the visible node list from the filesystem. Preserves the
    /// currently selected item by path. In git mode, filters to changed files
    /// only. In flat git mode, produces a single-level file list.
    pub(super) fn rebuild(&mut self) {
        let prev = self.nodes.get(self.tree_selected).map(|n| n.path.clone());
        let deleted = super::deleted_set(&self.git_status_map, self.git_show_deleted);

        if self.git_mode {
            if self.git_mode_flat {
                self.nodes = self.build_git_flat_nodes();
            } else {
                let all = build_visible(
                    &self.root,
                    &self.expanded,
                    self.show_hidden,
                    self.ignore_gitignore,
                    &deleted,
                );
                let map = &self.git_status_map;
                self.nodes = all
                    .into_iter()
                    .filter(|n| {
                        n.deleted || map.get(&n.path).is_some_and(|&s| s != GitStatus::Ignored)
                    })
                    .collect();
            }
        } else {
            self.nodes = build_visible(
                &self.root,
                &self.expanded,
                self.show_hidden,
                self.ignore_gitignore,
                &deleted,
            );
        }

        if let Some(p) = prev {
            if let Some(i) = self.nodes.iter().position(|n| n.path == p) {
                self.tree_selected = i;
                return;
            }
        }
        self.tree_selected = self.tree_selected.min(self.nodes.len().saturating_sub(1));
    }

    /// Produces a flat list of all changed (non-ignored) files with depth 0
    /// and their full relative path as the name, sorted alphabetically.
    fn build_git_flat_nodes(&self) -> Vec<TreeNode> {
        let mut entries: Vec<(PathBuf, bool)> = self
            .git_status_map
            .iter()
            .filter(|(path, &status)| {
                status != GitStatus::Ignored && path.starts_with(&self.root) && !path.is_dir()
            })
            .map(|(path, &status)| {
                let deleted = status == GitStatus::Deleted && !path.exists();
                (path.clone(), deleted)
            })
            .collect();
        entries.sort_by(|(a, _), (b, _)| a.cmp(b));
        entries
            .into_iter()
            .map(|(path, deleted)| {
                let name = path
                    .strip_prefix(&self.root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                TreeNode {
                    path,
                    name,
                    depth: 0,
                    is_dir: false,
                    deleted,
                }
            })
            .collect()
    }

    /// Expands all directories that contain git changes so they are visible in
    /// git mode's tree view.
    pub(super) fn expand_git_dirs(&mut self) {
        let dirs: Vec<PathBuf> = self
            .git_status_map
            .iter()
            .filter(|(path, &status)| {
                status != GitStatus::Ignored
                    && path.is_dir()
                    && path.starts_with(&self.root)
                    && **path != self.root
            })
            .map(|(p, _)| p.clone())
            .collect();
        for dir in dirs {
            self.expanded.insert(dir);
        }
    }

    /// Opens the tree node at `self.tree_selected` if it is a file (skips
    /// directories). Delegates to `show_deleted`, `show_working_tree_diff`, or
    /// `open_file` based on state.
    pub(super) fn try_open_selected(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                return;
            }
            if node.deleted {
                let path = node.path.clone();
                self.show_deleted(&path);
            } else if self.git_mode {
                let path = node.path.clone();
                self.show_working_tree_diff(&path);
            } else {
                let path = node.path.clone();
                self.open_file(&path);
            }
        }
    }

    /// Toggles git mode on/off. Enabling git mode fetches git status if needed,
    /// auto-expands changed directories, rebuilds the tree, and shows the
    /// working-tree diff for the selected file. Disabling restores the full
    /// tree and re-opens the current file as plain content.
    pub(super) fn toggle_git_mode(&mut self) {
        self.git_mode = !self.git_mode;
        self.config.git_mode = self.git_mode;
        if self.git_mode {
            // Ensure git status is populated even if git_status was disabled.
            if !self.git_status_enabled {
                self.git_status_enabled = true;
                self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
                self.git_branch = crate::git::current_branch(&self.root);
            }
            self.expand_git_dirs();
            self.rebuild();
            self.try_open_selected();
        } else {
            self.rebuild();
            // Re-open the current file as normal content instead of a diff.
            if let Some(path) = self.current_file.clone() {
                if self.is_diff {
                    self.open_file(&path);
                }
            }
        }
        self.save_config();
    }

    /// Acts on the currently selected node: toggles a directory's fold state,
    /// or opens a file. Shared by the Enter key and a mouse click.
    pub(super) fn activate_selected(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                let p = node.path.clone();
                if self.expanded.contains(&p) {
                    self.expanded.remove(&p);
                } else {
                    self.expanded.insert(p);
                }
                self.rebuild();
            } else if node.deleted {
                let p = node.path.clone();
                self.show_deleted(&p);
            } else if self.git_mode {
                let p = node.path.clone();
                self.show_working_tree_diff(&p);
            } else {
                let p = node.path.clone();
                self.open_file(&p);
            }
        }
    }

    /// Opens the currently selected search result and closes the overlay.
    /// Shared by the Enter key and a mouse click in the results list.
    pub(super) fn activate_search_selection(&mut self) {
        let action = self.search.as_ref().and_then(|s| match s.mode {
            SearchMode::Files => s.file_results.get(s.selected).map(|p| (p.clone(), None)),
            SearchMode::Content => s
                .content_results
                .get(s.selected)
                .map(|m| (m.path.clone(), Some(m.line_num))),
        });
        self.search = None;
        if let Some((path, line)) = action {
            self.open_file(&path);
            if let Some(ln) = line {
                self.content_scroll = ln.saturating_sub(1);
            }
            self.reveal_in_tree(&path.clone());
        }
    }
}
