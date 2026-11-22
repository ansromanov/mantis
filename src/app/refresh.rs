//! Per-frame update tick for `App`.
//!
//! `tick` runs once per render-loop iteration and drives all time- and
//! event-based updates: draining completed background loads, reloading an open
//! file when its watcher fires, advancing the debounced content search, and
//! refreshing the tree/git state. Tree refreshes are event-driven when the root
//! filesystem watcher is installed (reloading only after events go quiet for
//! `TREE_RELOAD_DEBOUNCE`, to coalesce bursts), with a periodic timer fallback
//! when no watcher could be installed so the view never goes permanently stale.
//! Config-file (`mantis.toml`) changes follow the same debounce before
//! `handle_config_change` runs, so an editor's atomic save doesn't trigger
//! more than one reload.

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
        if let Some(picker) = &mut self.worktree_picker {
            picker.poll_changed_counts();
        }
        self.drain_loads();
        self.drain_diagnostics();
        if let Some(latest) = self.update_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.new_version_available = Some(latest);
            self.update_rx = None;
        }
        if self.drain_config_watch() {
            self.config_dirty = true;
            self.config_dirty_at = Some(self.now());
        }
        let file_changed = self.drain_file_watch();
        if file_changed {
            if self.follow_mode {
                if let Some(ref path) = self.current_file.clone() {
                    if let Some(ref mut vf) = self.virtual_file {
                        let lines_before = vf.line_count();
                        match vf.update_growth(path) {
                            Some(true) => {
                                if vf.line_count() != lines_before {
                                    self.content_revision += 1;
                                    self.rebuild_filter_display_map();
                                    if self.follow_pinned {
                                        self.active_line =
                                            self.display_line_count().saturating_sub(1);
                                        self.scroll_active_line_into_view();
                                    }
                                }
                            }
                            Some(false) => {
                                self.reload_content();
                                self.set_status("file truncated — reloaded");
                            }
                            None => {}
                        }
                    } else {
                        self.reload_content();
                        if self.follow_pinned {
                            self.active_line = self.display_line_count().saturating_sub(1);
                            self.scroll_active_line_into_view();
                        }
                    }
                }
            } else if self.auto_watch {
                self.reload_content();
            }
        }
        if let Some(ref mut s) = self.search {
            s.maybe_refresh();
        }
        if let Some(ref mut p) = self.command_palette {
            if let Some(ref mut s) = p.route_search {
                s.maybe_refresh();
            }
        }
        let (root_changed, follow_path) = self.drain_root_watch();
        if root_changed {
            self.tree_dirty = true;
            self.tree_dirty_at = Some(self.now());
            if self.follow_mode {
                if let Some(path) = follow_path {
                    self.follow_candidate = Some(path);
                }
            }
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
        if self.config_dirty {
            // Wait for the config file to go quiet before reloading so an atomic
            // save (temp write + rename) produces one reload, not one per event.
            let quiet = self
                .config_dirty_at
                .is_some_and(|t| t.elapsed() >= Self::TREE_RELOAD_DEBOUNCE);
            if quiet {
                self.config_dirty = false;
                self.config_dirty_at = None;
                self.handle_config_change();
            }
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
                self.reload_tree();
                if self.follow_mode {
                    self.follow_latest_candidate();
                }
            }
        } else if self.root_watcher.is_none() && self.last_refresh.elapsed().as_secs() >= 30 {
            // No watcher (install failed): fall back to a blind periodic reload.
            self.reload();
        }
    }

    /// Opens the newest file observed by the root watcher when follow mode is
    /// active, using a working-tree diff inside a repository and plain content
    /// elsewhere. A paused follow session keeps the candidate for later.
    fn follow_latest_candidate(&mut self) {
        let Some(path) = self.follow_candidate.take() else {
            return;
        };
        if self
            .follow_paused_until
            .is_some_and(|until| until > self.now())
        {
            self.follow_candidate = Some(path);
            return;
        }
        if crate::git::repo_info(&self.root).is_some() {
            self.request_working_tree_diff(&path);
        } else {
            self.request_open_file(&path);
        }
        self.follow_pinned = true;
    }

    /// Detected a change to the config file: re-reads, validates, hot-reloads
    /// safe settings, and surfaces any errors in the status bar.
    pub(crate) fn handle_config_change(&mut self) {
        let Some(ref path) = self.config_path.clone() else {
            return;
        };
        let Ok(s) = std::fs::read_to_string(path) else {
            self.set_status("mantis.toml changed but could not be read");
            return;
        };
        let unknown = crate::config::validate::validate_keys(&s);
        match toml::from_str::<crate::config::Config>(&s) {
            Ok(mut cfg) => {
                cfg.migrate_legacy_flat_fields();
                cfg.migrate_legacy_git_fields();
                cfg.keys.migrate_legacy_keys();
                if !unknown.is_empty() {
                    self.set_status(format!("mantis.toml: {}", unknown.join("; ")));
                } else {
                    self.set_status("mantis.toml reloaded");
                }
                self.apply_reloaded_config(cfg);
            }
            Err(e) => {
                self.set_status(format!("mantis.toml parse error: {e}"));
            }
        }
    }

    /// Applies reloaded config fields to active App state, re-resolves the theme,
    /// and triggers a reload.
    pub(crate) fn apply_reloaded_config(&mut self, cfg: crate::config::Config) {
        self.show_hidden = cfg.tree.show_hidden;
        self.ignore_gitignore = cfg.git.ignore_gitignore;
        self.tree_width = cfg.tree.width;
        self.tree_independent_scroll = cfg.tree.independent_scroll;
        self.set_word_wrap(cfg.content.word_wrap);
        self.git_status_enabled = cfg.git.status;
        self.git_show_deleted = cfg.git.show_deleted;
        self.git_show_untracked = cfg.git.show_untracked;
        self.git_show_ignored = cfg.git.show_ignored;
        self.show_scrollbar = cfg.content.scrollbar;
        self.show_scroll_percentage = cfg.content.scroll_percentage;
        self.show_line_numbers = cfg.content.line_numbers;
        self.auto_watch = cfg.content.watch;
        self.show_file_info = cfg.content.show_file_info;
        self.indent_guides = cfg.tree.indent_guides;
        self.icons_enabled = cfg.tree.icons;
        self.keys = cfg.keys.clone();

        let theme_name = cfg.theme.name.as_deref().unwrap_or("default").to_string();
        let theme = cfg.theme.resolve();
        self.apply_theme(&theme_name, theme);

        self.config_baseline = cfg.clone();
        self.config = cfg;
        self.reload();
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
            LoadResponse::RangeStatus {
                seq,
                root,
                load,
                error,
            } => {
                if seq == self.git_seq && root == self.root {
                    self.apply_range_status_load(*load, error);
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
        self.git_staged_files = load.staged;
        self.git_unstaged_files = load.unstaged;
        self.git_untracked_files = load.untracked;
        self.git_info = load.info;
        if self.git_mode {
            self.expand_git_dirs();
            self.rebuild(false);
        }
    }

    /// Applies a range-status load (from `git diff --name-status <rev>`),
    /// used in compare mode. When in git mode, expands git dirs, rebuilds the
    /// tree so only changed files are shown, and opens the first selected
    /// file's diff (the tree was empty when compare mode was entered, so the
    /// initial `try_open_selected` had nothing to select). Surfaces `error`
    /// (e.g. an unknown revision) as a status message instead of silently
    /// leaving an empty tree.
    pub(super) fn apply_range_status_load(&mut self, load: GitStatusLoad, error: Option<String>) {
        if let Some(e) = error {
            self.set_status(format!("compare: {e}"));
            return;
        }
        self.git_status_map = load.status_map;
        self.git_staged_files = load.staged;
        self.git_unstaged_files = load.unstaged;
        self.git_untracked_files = load.untracked;
        self.git_info = load.info;
        if self.git_mode {
            self.expand_git_dirs();
            self.rebuild(false);
            self.try_open_selected();
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
    /// sequence so any in-flight worker result is treated as stale. When
    /// `compare_base` is set, the diff is computed against that revision instead
    /// of HEAD.
    pub(super) fn request_working_tree_diff(&mut self, path: &std::path::Path) {
        let seq = self.invalidate_pending_load();
        self.loading = true;
        self.loader.request(LoadRequest::Diff {
            seq,
            root: self.root.clone(),
            path: path.to_path_buf(),
            diff_mode: self.diff_mode,
            compare_base: self.compare_base.clone(),
        });
    }

    /// Enqueues a range-status refresh for compare mode via the background
    /// worker. Bumps `git_seq` so earlier in-flight results are ignored.
    pub(super) fn request_range_status(&mut self, rev: String) {
        self.git_seq = self.git_seq.wrapping_add(1);
        self.loader.request(LoadRequest::RangeStatus {
            seq: self.git_seq,
            root: self.root.clone(),
            rev,
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

    /// Removes one file's diagnostic data and its path ownership record.
    pub(crate) fn clear_diagnostics_for_path(&mut self, path: &std::path::Path) {
        self.file_diagnostics.remove(path);
        self.file_diagnostic_lines.remove(path);
        self.latest_diagnostic_requests.remove(path);
        if self.current_file.as_deref() == Some(path) {
            if let Some(picker) = self
                .command_palette
                .as_mut()
                .filter(|palette| {
                    palette.route == crate::command_palette::PaletteRoute::Diagnostics
                })
                .and_then(|palette| palette.route_diagnostics.as_mut())
            {
                picker.set_diagnostics(Vec::new());
            }
        }
    }

    /// Closes diagnostics tied to the previous file after genuine navigation.
    pub(crate) fn close_diagnostics_picker(&mut self) {
        if self.command_palette.as_ref().is_some_and(|palette| {
            palette.route == crate::command_palette::PaletteRoute::Diagnostics
        }) {
            self.command_palette = None;
        }
    }
}

#[cfg(test)]
#[path = "refresh_test.rs"]
mod tests;
