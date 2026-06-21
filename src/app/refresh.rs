//! Per-frame update tick for `App`.
//!
//! `tick` runs once per render-loop iteration and drives all time- and
//! event-based updates: draining completed background loads, reloading an open
//! file when its watcher fires, advancing the debounced content search, and
//! refreshing the tree/git state. Tree refreshes are event-driven when the root
//! filesystem watcher is installed (reloading only after events go quiet for
//! `TREE_RELOAD_DEBOUNCE`, to coalesce bursts), with a periodic timer fallback
//! when no watcher could be installed so the view never goes permanently stale.

use std::time::{Duration, Instant};

use super::loader::{LoadRequest, LoadResponse};
use super::App;

impl App {
    /// Per-frame update. Refreshes the open file from its watcher, advances the
    /// debounced content search, and drives the tree/git refresh: when the root
    /// watcher is installed this is event-driven (reload only after the tree has
    /// been quiet for `TREE_RELOAD_DEBOUNCE`); otherwise it falls back to a
    /// periodic reload so the view never goes permanently stale.
    pub fn tick(&mut self) {
        self.drain_loads();
        self.drain_plugin_actions();
        if self.auto_watch && self.drain_file_watch() {
            self.reload_content();
        }
        if let Some(ref mut s) = self.search {
            s.maybe_refresh();
        }
        if self.drain_root_watch() {
            self.tree_dirty = true;
            self.tree_dirty_at = Some(Instant::now());
        }
        if self.tree_dirty {
            // Wait for the tree to go quiet before reloading so a burst of events
            // produces one refresh, not one per event.
            let quiet = self
                .tree_dirty_at
                .is_some_and(|t| t.elapsed() >= Self::TREE_RELOAD_DEBOUNCE);
            if quiet {
                self.tree_dirty = false;
                self.tree_dirty_at = None;
                self.reload();
            }
        } else if self.root_watcher.is_none() && self.last_refresh.elapsed().as_secs() >= 30 {
            // No watcher (install failed): fall back to a blind periodic reload.
            self.reload();
        }
    }

    /// Drains all pending worker responses, applying the one matching the most
    /// recent `load_seq` and discarding superseded results. Returns `true` if a
    /// load was applied (so the caller knows to redraw).
    pub(super) fn drain_loads(&mut self) -> bool {
        // Collect first so the immutable borrow of `self.loader` is released
        // before `apply_*` takes `&mut self`.
        let responses: Vec<LoadResponse> =
            std::iter::from_fn(|| self.loader.rx.try_recv().ok()).collect();
        let mut applied = false;
        for resp in responses {
            match resp {
                LoadResponse::File { seq, path, load } => {
                    if seq == self.load_seq {
                        self.apply_file_load(&path, *load);
                        self.loading = false;
                        applied = true;
                    }
                }
                LoadResponse::Diff { seq, path, load } => {
                    if seq == self.load_seq {
                        self.apply_diff_load(&path, *load);
                        self.loading = false;
                        applied = true;
                    }
                }
            }
        }
        applied
    }

    /// Bumps the load sequence so any in-flight worker result is treated as
    /// stale, and clears the in-flight flag. Returns the new sequence number.
    /// Called by every operation that replaces the displayed content.
    pub(super) fn invalidate_pending_load(&mut self) -> u64 {
        self.load_seq = self.load_seq.wrapping_add(1);
        self.loading = false;
        self.load_seq
    }

    /// Dispatches a file open to the background worker (production) or runs it
    /// synchronously (tests, so assertions can observe content immediately).
    pub(super) fn request_open_file(&mut self, path: &std::path::Path) {
        if cfg!(test) {
            self.open_file(path);
        } else {
            let seq = self.invalidate_pending_load();
            self.loading = true;
            self.loader.request(LoadRequest::File {
                seq,
                path: path.to_path_buf(),
            });
        }
    }

    /// Dispatches a working-tree diff to the background worker (production) or
    /// runs it synchronously (tests).
    pub(super) fn request_working_tree_diff(&mut self, path: &std::path::Path) {
        if cfg!(test) {
            self.show_working_tree_diff(path);
        } else {
            let seq = self.invalidate_pending_load();
            self.loading = true;
            self.loader.request(LoadRequest::Diff {
                seq,
                root: self.root.clone(),
                path: path.to_path_buf(),
            });
        }
    }

    /// Opens the currently selected node synchronously. Used at startup so the
    /// first file renders on the first frame.
    pub(super) fn open_selected_sync(&mut self) {
        if let Some(node) = self.nodes.get(self.tree_selected) {
            if node.is_dir {
                return;
            }
            let path = node.path.clone();
            if node.deleted {
                self.show_deleted(&path);
            } else if self.git_mode {
                self.show_working_tree_diff(&path);
            } else {
                self.open_file(&path);
            }
        }
    }

    /// Debounce window: how long the tree must stay quiet after a filesystem
    /// event before a reload runs.
    #[cfg(not(test))]
    const TREE_RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
    #[cfg(test)]
    const TREE_RELOAD_DEBOUNCE: Duration = Duration::from_secs(60);

    /// Tells the worker to rebuild its highlighter/theme after a theme change.
    pub(super) fn loader_set_theme(&self) {
        self.loader
            .request(LoadRequest::SetTheme(Box::new(self.theme.clone())));
    }

    /// Drains pending plugin actions and handles known action types.
    fn drain_plugin_actions(&mut self) {
        if self.plugin_manager.is_empty() {
            return;
        }
        self.plugin_manager.drain_actions();
        for (name, action, params) in self.plugin_manager.take_actions() {
            match action.as_str() {
                "show_message" => {
                    if let Some(msg) = params.get("message").and_then(|v| v.as_str()) {
                        self.plugin_message = Some(format!("[{name}] {msg}"));
                    }
                }
                "open_file" => {
                    if let Some(path_str) = params.get("path").and_then(|v| v.as_str()) {
                        self.open_and_reveal(std::path::Path::new(path_str));
                    }
                }
                "set_file_statuses" => {
                    if let Some(obj) = params.as_object() {
                        for (path_str, status_val) in obj {
                            let Some(status_str) = status_val.as_str() else {
                                continue;
                            };
                            let git_status = match status_str {
                                "modified" | "renamed" | "conflict" => {
                                    crate::git::GitStatus::Modified
                                }
                                "added" | "untracked" => crate::git::GitStatus::New,
                                "deleted" => crate::git::GitStatus::Deleted,
                                "ignored" => crate::git::GitStatus::Ignored,
                                _ => continue,
                            };
                            self.git_status_map
                                .insert(std::path::PathBuf::from(path_str), git_status);
                        }
                    }
                }
                "set_blame_data" => {
                    let path = match params.get("path").and_then(|v| v.as_str()) {
                        Some(p) => std::path::PathBuf::from(p),
                        None => continue,
                    };
                    let lines: Vec<String> = match params.get("lines").and_then(|v| v.as_array()) {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                        None => continue,
                    };
                    self.plugin_blame.insert(path, lines);
                }
                "set_icon_map" => {
                    if let Some(obj) = params.as_object() {
                        if let Some(icons) = obj.get("icons").and_then(|v| v.as_object()) {
                            for (ext, glyph) in icons {
                                if let Some(g) = glyph.as_str() {
                                    self.icon_map
                                        .insert(ext.to_ascii_lowercase(), g.to_string());
                                }
                            }
                        }
                        if let Some(open) = obj.get("dir_open").and_then(|v| v.as_str()) {
                            self.icon_dir_open = open.to_string();
                        }
                        if let Some(closed) = obj.get("dir_closed").and_then(|v| v.as_str()) {
                            self.icon_dir_closed = closed.to_string();
                        }
                        if let Some(fallback) = obj.get("fallback").and_then(|v| v.as_str()) {
                            self.icon_fallback = fallback.to_string();
                        }
                    }
                }
                "set_status_bar_git_info" => {
                    let branch = params
                        .get("branch")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let head = params
                        .get("head")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let dirty = params
                        .get("dirty")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let state = params
                        .get("state")
                        .and_then(|v| v.as_str())
                        .unwrap_or("clean")
                        .to_string();
                    self.plugin_git_info = Some(super::PluginGitInfo {
                        branch,
                        head,
                        dirty,
                        state,
                    });
                }
                "set_content" => {
                    let lines: Vec<String> = match params.get("lines").and_then(|v| v.as_array()) {
                        Some(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                        None => continue,
                    };
                    self.markdown_lines = lines
                        .iter()
                        .map(|l| crate::ansi::parse_ansi_line(l))
                        .collect();
                    if let Some(path_str) = params.get("path").and_then(|v| v.as_str()) {
                        self.current_file = Some(std::path::PathBuf::from(path_str));
                    }
                    self.content_scroll = 0;
                    self.content_hscroll = 0;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
#[path = "refresh_test.rs"]
mod tests;
