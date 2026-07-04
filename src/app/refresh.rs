//! Per-frame update tick for `App`.
//!
//! `tick` runs once per render-loop iteration and drives all time- and
//! event-based updates: draining completed background loads, reloading an open
//! file when its watcher fires, advancing the debounced content search, and
//! refreshing the tree/git state. Tree refreshes are event-driven when the root
//! filesystem watcher is installed (reloading only after events go quiet for
//! `TREE_RELOAD_DEBOUNCE`, to coalesce bursts), with a periodic timer fallback
//! when no watcher could be installed so the view never goes permanently stale.

use std::time::Duration;

use super::loader::{GitStatusLoad, LoadRequest, LoadResponse};
use super::App;

impl App {
    /// How long a status message lingers before auto-expiring (3 seconds).
    const STATUS_TTL: Duration = Duration::from_secs(3);

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
            self.tree_dirty_at = Some(self.now());
        }
        // Debounced session save: persist 2 s after the last state change.
        if self.session_dirty {
            let quiet = self
                .session_dirty_at
                .is_some_and(|t| t.elapsed() >= Duration::from_secs(2));
            if quiet {
                self.save_session();
            }
        }
        // Auto-expire status message after TTL.
        if self
            .status_message
            .as_ref()
            .is_some_and(|sm| sm.expired(Self::STATUS_TTL))
        {
            self.status_message = None;
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

    /// Drains all pending worker responses, applying those matching the most
    /// recent sequence numbers and discarding superseded results. Returns
    /// `true` if a file/diff load was applied (so the caller knows to redraw).
    pub(super) fn drain_loads(&mut self) -> bool {
        // Collect first so the immutable borrow of `self.loader` is released
        // before `apply_response` takes `&mut self`.
        let responses: Vec<LoadResponse> =
            std::iter::from_fn(|| self.loader.rx.try_recv().ok()).collect();
        let mut applied = false;
        for resp in responses {
            applied |= self.apply_response(resp);
        }
        applied
    }

    /// Applies a single worker response, checking seq/root for staleness.
    /// Returns `true` when a file or diff load was applied.
    pub(super) fn apply_response(&mut self, resp: LoadResponse) -> bool {
        match resp {
            LoadResponse::File { seq, path, load } => {
                if seq == self.load_seq {
                    self.apply_file_load(&path, *load);
                    self.loading = false;
                    return true;
                }
            }
            LoadResponse::Diff { seq, path, load } => {
                if seq == self.load_seq {
                    self.apply_diff_load(&path, *load);
                    self.loading = false;
                    return true;
                }
            }
            LoadResponse::GitStatus { seq, root, load } => {
                if seq == self.git_seq && root == self.root {
                    self.apply_git_status_load(*load);
                }
            }
            #[cfg(test)]
            LoadResponse::Barrier(_) => {}
        }
        false
    }

    /// Applies a [`GitStatusLoad`] to the app state, updating the status map
    /// and info. When in git mode, expand git dirs and rebuild the tree so
    /// colors and filtering are current.
    pub(super) fn apply_git_status_load(&mut self, load: GitStatusLoad) {
        self.git_status_map = load.status_map;
        self.git_info = load.info;
        if self.git_mode {
            self.expand_git_dirs();
            self.rebuild(false);
        }
    }

    /// Bumps the load sequence so any in-flight worker result is treated as
    /// stale, and clears the in-flight flag. Returns the new sequence number.
    /// Called by every operation that replaces the displayed content.
    pub(super) fn invalidate_pending_load(&mut self) -> u64 {
        self.load_seq = self.load_seq.wrapping_add(1);
        self.loading = false;
        self.load_seq
    }

    /// Dispatches a file open to the background worker. Bumps the load
    /// sequence so any in-flight worker result is treated as stale.
    pub(super) fn request_open_file(&mut self, path: &std::path::Path) {
        let seq = self.invalidate_pending_load();
        self.loading = true;
        self.loader.request(LoadRequest::File {
            seq,
            path: path.to_path_buf(),
        });
    }

    /// Dispatches a working-tree diff to the background worker. Bumps the load
    /// sequence so any in-flight worker result is treated as stale.
    pub(super) fn request_working_tree_diff(&mut self, path: &std::path::Path) {
        let seq = self.invalidate_pending_load();
        self.loading = true;
        self.loader.request(LoadRequest::Diff {
            seq,
            root: self.root.clone(),
            path: path.to_path_buf(),
            diff_mode: self.diff_mode,
        });
    }

    /// Enqueues a git-status refresh via the background worker. Bumps
    /// `git_seq` so earlier in-flight results are ignored.
    pub(super) fn request_git_status_refresh(&mut self) {
        let effective_show_ignored = self.git_show_ignored || self.ignore_gitignore;
        self.git_seq = self.git_seq.wrapping_add(1);
        self.loader.request(LoadRequest::GitStatus {
            seq: self.git_seq,
            root: self.root.clone(),
            include_untracked: self.git_show_untracked,
            include_ignored: effective_show_ignored,
        });
    }

    /// Blocks the current thread until the worker thread has processed all
    /// requests queued before this call and their results have been applied.
    /// Only available in tests so assertions can observe content/git-state
    /// immediately after a `request_*` call.
    ///
    /// Sends a [`LoadRequest::Barrier`] and applies every response that
    /// arrives before its echo: since the worker's request channel is FIFO
    /// and single-threaded, seeing the echo guarantees every prior request
    /// has already been applied. This is deterministic rather than
    /// silence-based, so it can't race a worker that's still busy. The
    /// timeout is only a safety net against a wedged worker thread.
    #[cfg(test)]
    pub(crate) fn pump_loads(&mut self) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_BARRIER: AtomicU64 = AtomicU64::new(0);
        let token = NEXT_BARRIER.fetch_add(1, Ordering::Relaxed);
        self.loader.request(LoadRequest::Barrier(token));
        loop {
            match self.loader.rx.recv_timeout(Duration::from_secs(5)) {
                Ok(LoadResponse::Barrier(t)) if t == token => break,
                Ok(resp) => {
                    self.apply_response(resp);
                }
                Err(_) => break,
            }
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

    /// Tells the worker to rebuild its highlighter with updated syntax definitions.
    pub(super) fn loader_set_extra_syntaxes(&self) {
        self.loader
            .request(LoadRequest::SetExtraSyntaxes(self.extra_syntaxes.clone()));
    }

    /// Drains pending plugin actions and handles known action types.
    fn drain_plugin_actions(&mut self) {
        if self.plugin_manager.is_empty() {
            return;
        }
        self.plugin_manager.drain_actions();
        for (name, action, params) in self.plugin_manager.take_actions() {
            self.handle_plugin_action(&name, &action, &params);
        }
        // Tear down contributions of any plugins that exited or crashed.
        for name in self.plugin_manager.take_dead_plugins() {
            self.plugin_message = Some(format!(
                "Plugin '{name}' exited unexpectedly; tearing down its state."
            ));
            self.teardown_plugin_contributions(&name);
        }
    }

    /// Handles a single plugin action. Extracted from the drain loop so tests
    /// can exercise the production code path directly instead of duplicating it.
    pub(crate) fn handle_plugin_action(
        &mut self,
        name: &str,
        action: &str,
        params: &serde_json::Value,
    ) {
        match action {
            "show_message" => self.handle_plugin_show_message(name, params),
            "open_file" => self.handle_plugin_open_file(name, params),
            "set_icon_map" => self.handle_plugin_set_icon_map(name, params),
            "set_content" => self.handle_plugin_set_content(name, params),
            "register_language_provider" => {
                self.handle_plugin_register_language_provider(name, params);
            }
            "set_fold_regions" => self.handle_plugin_set_fold_regions(name, params),
            _ => {}
        }
    }

    fn handle_plugin_show_message(&mut self, name: &str, params: &serde_json::Value) {
        if let Some(msg) = params.get("message").and_then(|v| v.as_str()) {
            self.plugin_message = Some(format!("[{name}] {msg}"));
        }
    }

    fn handle_plugin_open_file(&mut self, _name: &str, params: &serde_json::Value) {
        if let Some(path_str) = params.get("path").and_then(|v| v.as_str()) {
            self.plugin_is_opening_file = true;
            self.open_and_reveal(std::path::Path::new(path_str));
            self.plugin_is_opening_file = false;
        }
    }

    fn handle_plugin_set_icon_map(&mut self, name: &str, params: &serde_json::Value) {
        if let Some(obj) = params.as_object() {
            if let Some(icons) = obj.get("icons").and_then(|v| v.as_object()) {
                for (ext, glyph) in icons {
                    if let Some(g) = glyph.as_str() {
                        self.icon_map
                            .insert(ext.to_ascii_lowercase(), g.to_string());
                    }
                }
                self.icons_enabled = true;
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
            self.plugin_contributions
                .entry(name.to_string())
                .or_default()
                .has_icon_map = true;
        }
    }

    fn handle_plugin_set_content(&mut self, name: &str, params: &serde_json::Value) {
        let lines: Vec<String> = match params.get("lines").and_then(|v| v.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
            None => return,
        };
        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => std::path::PathBuf::from(p),
            None => return,
        };
        let rendered: Vec<Vec<(ratatui::style::Style, String)>> = lines
            .iter()
            .map(|l| crate::ansi::parse_ansi_line(l))
            .collect();
        let text: Vec<String> = rendered
            .iter()
            .map(|spans| spans.iter().map(|(_, t)| t.as_str()).collect::<String>())
            .collect();
        // Only reset scroll / mark active when the render targets the
        // file currently on screen; a plugin rendering a background path
        // must not yank the viewport of the file the user is reading.
        let is_current = self.current_file.as_deref() == Some(path.as_path());
        self.plugin_content_text.insert(path.clone(), text);
        self.plugin_content.insert(path.clone(), rendered);
        self.plugin_contributions
            .entry(name.to_string())
            .or_default()
            .content_paths
            .insert(path);
        if is_current {
            // First render of this file by a plugin resets the viewport;
            // subsequent re-renders preserve scroll (clamped) so that
            // periodic plugin updates don't yank the user's position.
            let first_render =
                self.plugin_content_active_path.as_deref() != self.current_file.as_deref();
            self.plugin_content_active_path = self.current_file.clone();
            if first_render {
                self.set_content_scroll(0);
                self.content_hscroll = 0;
            } else {
                self.clamp_content_scroll();
            }
            self.plugin_content_active = true;
        }
    }

    fn handle_plugin_register_language_provider(&mut self, name: &str, params: &serde_json::Value) {
        let extensions: Vec<String> = params
            .get("extensions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_ascii_lowercase))
                    .collect()
            })
            .unwrap_or_default();
        let capabilities: std::collections::HashSet<crate::plugin::Capability> = params
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        let reg = crate::plugin::LanguageProviderRegistration {
            plugin_name: name.to_string(),
            extensions,
            capabilities,
        };
        self.plugin_manager.register_provider(reg);
    }

    fn handle_plugin_set_fold_regions(&mut self, name: &str, params: &serde_json::Value) {
        let path = match params.get("path").and_then(|v| v.as_str()) {
            Some(p) => std::path::PathBuf::from(p),
            None => return,
        };
        // Only accept fold regions from a provider that registered the
        // file's extension with the Fold capability.
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if self
            .plugin_manager
            .provider_for(ext, &crate::plugin::Capability::Fold)
            .is_none()
        {
            return;
        }
        let regions: Vec<crate::fold::FoldRegion> = params
            .get("regions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        let pair = r.as_array()?;
                        let start = pair.first()?.as_i64()? as usize;
                        let end = pair.get(1)?.as_i64()? as usize;
                        Some(crate::fold::FoldRegion { start, end })
                    })
                    .collect()
            })
            .unwrap_or_default();
        self.plugin_fold_regions.insert(path.clone(), regions);
        self.plugin_contributions
            .entry(name.to_string())
            .or_default()
            .fold_region_paths
            .insert(path.clone());
        if self.current_file.as_deref() == Some(&path) {
            self.apply_plugin_fold_regions(&path);
        }
    }
}

#[cfg(test)]
#[path = "refresh_test.rs"]
mod tests;
