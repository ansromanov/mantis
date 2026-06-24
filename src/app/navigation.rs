//! Tree navigation and rebuilding for `App`.
//!
//! `rebuild` regenerates the visible `Vec<TreeNode>` from the filesystem while
//! preserving the selected entry by path; in git mode it filters to changed
//! files (hierarchical or flat). The movement helpers move the selection up and
//! down, expand and collapse directories, jump to the top/bottom, and keep the
//! viewport scrolled so the selection stays visible. These methods are the
//! supported way to mutate tree selection and expansion state, so geometry and
//! git-mode invariants stay consistent across keyboard and mouse input rather
//! than being poked at from multiple call sites.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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
                self.walk_errors = 0;
                self.nodes = self.build_git_flat_nodes();
            } else {
                let (all, errs) = build_visible(
                    &self.root,
                    &self.expanded,
                    self.show_hidden,
                    self.ignore_gitignore,
                    &deleted,
                );
                self.walk_errors = errs;
                let map = &self.git_status_map;
                self.nodes = all
                    .into_iter()
                    .filter(|n| {
                        n.deleted || map.get(&n.path).is_some_and(|&s| s != GitStatus::Ignored)
                    })
                    .collect();
            }
        } else {
            let (nodes, errs) = build_visible(
                &self.root,
                &self.expanded,
                self.show_hidden,
                self.ignore_gitignore,
                &deleted,
            );
            self.walk_errors = errs;
            self.nodes = nodes;
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
    /// `open_file` based on state. Notifies plugins of the selection change.
    pub(super) fn try_open_selected(&mut self) {
        let path = self.nodes.get(self.tree_selected).map(|n| n.path.clone());
        let Some(ref path) = path else {
            return;
        };
        let is_dir = self.nodes.get(self.tree_selected).is_some_and(|n| n.is_dir);
        if is_dir {
            return;
        }
        let deleted = self
            .nodes
            .get(self.tree_selected)
            .is_some_and(|n| n.deleted);
        if deleted {
            self.show_deleted(path);
        } else if self.git_mode {
            self.request_working_tree_diff(path);
        } else {
            self.request_open_file(path);
        }
        self.plugin_manager.on_selection_change(Some(path));
    }

    /// Toggles git mode on/off. Enabling git mode fetches git status if needed,
    /// auto-expands changed directories, rebuilds the tree, and shows the
    /// working-tree diff for the selected file. Disabling restores the full
    /// tree and re-opens the current file as plain content.
    pub(super) fn toggle_git_mode(&mut self) {
        self.git_mode = !self.git_mode;
        self.config.git_mode = self.git_mode;
        self.mark_session_dirty();
        if self.git_mode {
            // Ensure git status is populated even if git_status was disabled.
            if !self.git_status_enabled {
                self.git_status_enabled = true;
                #[cfg(feature = "git-core")]
                {
                    self.git_status_map =
                        crate::git::repo_status(&self.root, self.ignore_gitignore);
                    self.git_info = crate::git::repo_info(&self.root);
                }
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
        let Some(node) = self.nodes.get(self.tree_selected) else {
            return;
        };
        if node.is_dir {
            let p = node.path.clone();
            if self.expanded.contains(&p) {
                self.expanded.remove(&p);
            } else {
                self.expanded.insert(p);
            }
            self.mark_session_dirty();
            self.rebuild();
        } else {
            let p = node.path.clone();
            let deleted = node.deleted;
            if deleted {
                self.show_deleted(&p);
            } else if self.git_mode {
                self.request_working_tree_diff(&p);
            } else {
                self.request_open_file(&p);
            }
            self.plugin_manager.on_selection_change(Some(&p));
        }
    }

    /// Expands all ancestor directories of `path` and selects the file in the
    /// tree so it becomes visible. Used by `open_and_reveal` and search results.
    pub(super) fn reveal_in_tree(&mut self, path: &Path) {
        let mut current = path.parent();
        while let Some(dir) = current {
            if dir == self.root {
                break;
            }
            if dir.starts_with(&self.root) {
                self.expanded.insert(dir.to_path_buf());
            } else {
                break;
            }
            current = dir.parent();
        }
        self.rebuild();
        if let Some(i) = self.nodes.iter().position(|n| n.path == path) {
            self.tree_selected = i;
            // Keep the viewport on the revealed node in independent-scroll mode;
            // otherwise the selection can land outside the stale viewport and
            // render unhighlighted. No-op when independent scroll is off.
            self.scroll_tree_into_view();
        }
    }

    /// Navigates the tree to the directory at `path`. When `path` is within the
    /// current root, it expands ancestors and selects the node. When `path` is
    /// above the root (e.g. a parent directory clicked in the breadcrumb), the
    /// root is changed to that path so the tree shows that directory's contents.
    /// Called when a breadcrumb segment is clicked.
    pub(super) fn navigate_to_breadcrumb(&mut self, path: &std::path::Path) {
        // Three cases, in order:
        //   1. path == root  → select index 0 in place.
        //   2. path above root → change the viewer root so the tree shows that dir.
        //   3. path inside root → expand ancestors and select the node.
        if path == self.root {
            self.tree_selected = 0;
            self.scroll_tree_into_view();
            return;
        }
        if !path.starts_with(&self.root) {
            self.set_root(path);
            return;
        }
        // Expand all ancestors of the target directory.
        let mut current = path.parent();
        while let Some(dir) = current {
            if dir == self.root {
                break;
            }
            if dir.starts_with(&self.root) {
                self.expanded.insert(dir.to_path_buf());
            } else {
                break;
            }
            current = dir.parent();
        }
        self.expanded.insert(path.to_path_buf());
        self.rebuild();
        if let Some(i) = self.nodes.iter().position(|n| n.path == path) {
            self.tree_selected = i;
            self.scroll_tree_into_view();
        }
    }

    /// Changes the viewer root to `path`, rebuilding the tree and resetting
    /// content state. Clears the current file, reselects the root node, and
    /// re-fetches git status when the feature is enabled.
    ///
    /// # Maintenance note
    /// Every field below that holds per-file view state must stay in sync with
    /// `App`. When you add a new field to `App` that caches file or view state,
    /// add a reset here or verify that its default value is correct after a root
    /// change.
    fn set_root(&mut self, path: &std::path::Path) {
        self.root = path.to_path_buf();
        self.expanded.clear();
        self.current_file = None;
        self.content = Vec::new();
        self.highlighted = Vec::new();
        self.markdown_lines = Vec::new();
        self.virtual_file = None;
        self.is_markdown = false;
        self.is_json = false;
        self.file_encoding = None;
        self.file_line_ending = None;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.is_diff = false;
        self.diff_side_by_side = false;
        self.diff_rows = Vec::new();
        self.content_title = None;
        self.selection = None;
        self.visual_line = None;
        self.blame_panel = false;
        self.drag_start = None;
        self.fold_regions = Vec::new();
        self.folded = HashSet::new();
        self.fold_display_map = Vec::new();
        self.yaml_error = None;
        self.yaml_anchor_count = 0;
        self.yaml_alias_count = 0;
        self.file_watcher = None;
        self.file_watch_rx = None;
        self.file_watch_path = None;
        self.in_file_search = None;
        self.plugin_content_active = false;
        self.plugin_content.clear();
        self.plugin_content_text.clear();
        self.plugin_blame.clear();
        self.plugin_git_info = None;
        self.load_seq = self.load_seq.wrapping_add(1);
        #[cfg(feature = "git-core")]
        if self.git_status_enabled {
            self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
            self.git_info = crate::git::repo_info(&self.root);
        }
        #[cfg(not(feature = "git-core"))]
        {
            self.git_status_map.clear();
            self.git_info = None;
        }
        if self.git_mode {
            self.expand_git_dirs();
        }
        self.rebuild();
        self.tree_selected = 0;
        self.scroll_tree_into_view();
        if let Some(node) = self.nodes.get(self.tree_selected) {
            self.plugin_manager
                .on_selection_change(Some(node.path.as_path()));
        }
    }

    /// Collapses every expanded directory, resetting the tree to its top-level
    /// view. When the previously selected path is no longer visible (it was
    /// nested under a collapsed directory), the nearest visible ancestor
    /// directory is selected instead of falling back to an arbitrary index.
    pub(super) fn collapse_all(&mut self) {
        let prev_path = self.nodes.get(self.tree_selected).map(|n| n.path.clone());
        self.expanded.clear();
        self.mark_session_dirty();
        self.rebuild();
        // rebuild() preserves the selection when the path is still visible.
        // When the path is hidden (was nested), walk up to the nearest ancestor
        // that is now visible so the user lands on a related entry.
        if let Some(ref path) = prev_path {
            if self.nodes.get(self.tree_selected).map(|n| &n.path) != Some(path) {
                let mut ancestor = path.parent();
                while let Some(dir) = ancestor {
                    if let Some(i) = self.nodes.iter().position(|n| n.path == dir) {
                        self.tree_selected = i;
                        break;
                    }
                    if dir == self.root {
                        break;
                    }
                    ancestor = dir.parent();
                }
            }
        }
        self.scroll_tree_into_view();
        if let Some(i) = self
            .nodes
            .get(self.tree_selected)
            .map(|_| self.tree_selected)
        {
            let path = self.nodes[i].path.as_path();
            self.plugin_manager.on_selection_change(Some(path));
        }
    }

    /// Expands every directory in the tree so all files are visible. The
    /// selection is preserved by path across the rebuild.
    pub(super) fn expand_all(&mut self) {
        let dirs =
            crate::tree::collect_all_dirs(&self.root, self.show_hidden, self.ignore_gitignore);
        for dir in dirs {
            self.expanded.insert(dir);
        }
        self.mark_session_dirty();
        self.rebuild();
        self.scroll_tree_into_view();
        if let Some(i) = self
            .nodes
            .get(self.tree_selected)
            .map(|_| self.tree_selected)
        {
            let path = self.nodes[i].path.as_path();
            self.plugin_manager.on_selection_change(Some(path));
        }
    }

    /// Changes the viewer root to the currently selected node if it is a
    /// directory. Called when the user double-clicks a directory in the tree
    /// to descend into it as if `tv` were launched there.
    pub(super) fn descend_to_selected(&mut self) {
        let path = self
            .nodes
            .get(self.tree_selected)
            .filter(|n| n.is_dir)
            .map(|n| n.path.clone());
        if let Some(p) = path {
            self.set_root(&p);
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

#[cfg(test)]
#[path = "navigation_test.rs"]
mod tests;
