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
//!
//! `tick` also resolves any `pending_keypress` (protocol 3+ `on_keypress` key
//! consumption): once a `key_handled` reply arrives or the deadline passes,
//! `process_pending_keypress` either swallows the key or falls through to
//! normal-mode handling, following the same deferred/debounced pattern as the
//! search debounce below. `handle_plugin_action` grew two protocol 3 cases:
//! `key_handled` (feeds that resolution) and `plugin_error` (recorded via
//! `PluginManager` and logged, distinct from routine `show_message` text).

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
        self.check_overlay_transitions();
        self.drain_loads();
        if let Some(latest) = self.update_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.new_version_available = Some(latest);
            self.update_rx = None;
        }
        self.drain_plugin_actions();
        self.process_pending_keypress();
        if self.drain_config_watch() {
            self.config_dirty = true;
            self.config_dirty_at = Some(self.now());
        }
        if self.auto_watch && self.drain_file_watch() {
            self.reload_content();
        }
        if let Some(ref mut s) = self.search {
            s.maybe_refresh();
        }
        if let Some(ref mut p) = self.command_palette {
            if let Some(ref mut s) = p.route_search {
                s.maybe_refresh();
            }
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
                self.reload();
            }
        } else if self.root_watcher.is_none() && self.last_refresh.elapsed().as_secs() >= 30 {
            // No watcher (install failed): fall back to a blind periodic reload.
            self.reload();
        }
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
                cfg.migrate_legacy_plugin_paths();
                cfg.keys.migrate_legacy_keys();
                let retired = crate::plugin::retired_bundled_plugins();
                cfg.plugins.retain(|_name, entry| {
                    let Some(fname) = entry.path.file_name().and_then(|s| s.to_str()) else {
                        return true;
                    };
                    !retired.contains(&fname)
                });
                let plugins_changed = self.config.plugins != cfg.plugins;
                if plugins_changed {
                    self.set_status("mantis.toml changed — restart to apply");
                } else if !unknown.is_empty() {
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
        self.word_wrap = cfg.content.word_wrap;
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
        // Only apply the config default when no active plugin currently owns
        // the icon map — otherwise a config-file reload (e.g. the app's own
        // atomic `save_config` write triggering the watcher) would stomp the
        // `set_icon_map` action's live `icons_enabled = true` right back to
        // `cfg.tree.icons`'s cold-start default of `false`.
        if !self.plugin_contributions.values().any(|c| c.has_icon_map) {
            self.icons_enabled = cfg.tree.icons;
        }
        self.keys = cfg.keys.clone();

        let theme_name = cfg.theme.name.as_deref().unwrap_or("default").to_string();
        let theme = cfg.theme.resolve();
        self.apply_theme(&theme_name, theme);

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
        self.git_status_map = load.status_map;
        self.git_info = load.info;
        if self.git_mode {
            self.expand_git_dirs();
            self.rebuild(false);
            self.try_open_selected();
        }
        if let Some(e) = error {
            self.set_status(format!("compare: {e}"));
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

    /// Tells the worker to rebuild its highlighter with updated syntax definitions.
    pub(super) fn loader_set_extra_syntaxes(&self) {
        self.loader
            .request(LoadRequest::SetExtraSyntaxes(self.extra_syntaxes.clone()));
    }

    /// Resolves `pending_keypress` (protocol 3+ `on_keypress` key
    /// consumption), called once per tick after `drain_plugin_actions` so a
    /// `key_handled` reply received this tick is seen immediately.
    ///
    /// If `pending_keypress_handled` was set (a subscribed plugin replied
    /// `key_handled: true` for this key), the key is swallowed — cleared
    /// without running normal-mode handling. Otherwise, once the deadline
    /// passes, it falls through to `handle_normal_key` exactly as it would
    /// without any subscriber. A key still within its window is left pending.
    pub(crate) fn process_pending_keypress(&mut self) {
        let Some(deadline) = self.pending_keypress.as_ref().map(|p| p.deadline) else {
            return;
        };
        if self.pending_keypress_handled {
            self.pending_keypress = None;
            self.pending_keypress_handled = false;
        } else if deadline <= self.now() {
            if let Some(pending) = self.pending_keypress.take() {
                self.handle_normal_key(pending.key);
            }
        }
    }

    /// Immediately resolves a stale `pending_keypress` by falling through to
    /// normal-mode handling, without waiting for its deadline. Called when a
    /// new keypress arrives while one is still outstanding (e.g. rapid
    /// typing within one tick), so no keystroke is dropped waiting on a
    /// plugin reply that will never reference this new key.
    pub(crate) fn preempt_pending_keypress(&mut self) {
        if let Some(pending) = self.pending_keypress.take() {
            self.pending_keypress_handled = false;
            self.handle_normal_key(pending.key);
        }
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
            let detail = match self.plugin_manager.crash_detail(&name) {
                Some(info) => match (&info.last_stderr, &info.log_path) {
                    (Some(line), Some(path)) => {
                        format!(" (last stderr: {line}; full log: {})", path.display())
                    }
                    (Some(line), None) => format!(" (last stderr: {line})"),
                    (None, Some(path)) => format!(" (full log: {})", path.display()),
                    (None, None) => String::new(),
                },
                None => String::new(),
            };
            self.plugin_message = Some(format!(
                "Plugin '{name}' exited unexpectedly{detail}; tearing down its state."
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
            "register_commands" => self.handle_plugin_register_commands(name, params),
            "key_handled" => self.handle_plugin_key_handled(params),
            "plugin_error" => self.handle_plugin_error(name, params),
            _ => {}
        }
    }

    /// Handles a `key_handled` action (protocol 3+): if `handled: true`,
    /// marks the current `pending_keypress` as claimed so
    /// `process_pending_keypress` swallows it on the next tick instead of
    /// falling through to normal-mode handling. A reply that arrives with no
    /// keypress pending (e.g. a stray or duplicate reply) is a harmless noop.
    fn handle_plugin_key_handled(&mut self, params: &serde_json::Value) {
        if self.pending_keypress.is_some()
            && params.get("handled").and_then(|v| v.as_bool()) == Some(true)
        {
            self.pending_keypress_handled = true;
        }
    }

    /// Handles a `plugin_error` action (protocol 3+): records it in the
    /// plugin's rotating stderr log and via `PluginManager::record_plugin_error`
    /// (so the plugin picker can badge it), and shows it in the status bar
    /// with error styling — distinct from routine `show_message` text, which
    /// this is not treated as.
    fn handle_plugin_error(&mut self, name: &str, params: &serde_json::Value) {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error")
            .to_string();
        let context = params
            .get("context")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        let log_line = match &context {
            Some(c) => format!("[plugin_error] {c}: {message}"),
            None => format!("[plugin_error] {message}"),
        };
        self.plugin_manager.log_plugin_error_line(name, &log_line);
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                module: "plugin",
                kind: "plugin_error",
            });
        self.plugin_manager
            .record_plugin_error(name, message.clone(), context.clone());
        self.plugin_error = Some(match &context {
            Some(c) => format!("[{name}] {message} ({c})"),
            None => format!("[{name}] {message}"),
        });
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
        let priority = params.get("priority").and_then(|v| v.as_i64()).unwrap_or(0);
        let reg = crate::plugin::LanguageProviderRegistration {
            plugin_name: name.to_string(),
            extensions,
            capabilities,
            priority,
        };
        if let Some(warning) = self.plugin_manager.register_provider(reg) {
            self.plugin_message = Some(warning);
        }
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

    /// Handles a `register_commands` action: parses the command list, stores
    /// it in the `PluginManager`, and records the ids in the plugin's
    /// contributions so teardown can remove exactly this plugin's commands.
    /// An empty or missing list clears the plugin's registration. Ids that
    /// collide with a built-in action id or a command already registered by
    /// another plugin are skipped — otherwise the built-in dispatch arm would
    /// shadow the plugin entry, or ownership would be ambiguous.
    fn handle_plugin_register_commands(&mut self, name: &str, params: &serde_json::Value) {
        let commands: Vec<crate::plugin::PluginCommand> = params
            .get("commands")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| {
                        serde_json::from_value::<crate::plugin::PluginCommand>(v.clone()).ok()
                    })
                    .filter(|c: &crate::plugin::PluginCommand| {
                        let builtin = crate::actions::ACTIONS.iter().any(|a| a.id == c.id);
                        let owned_elsewhere = self
                            .plugin_manager
                            .plugin_for_command(&c.id)
                            .is_some_and(|owner| owner != name);
                        if builtin || owned_elsewhere {
                            self.plugin_manager.log_plugin_error_line(
                                name,
                                &format!(
                                    "register_commands: skipping id {:?} ({})",
                                    c.id,
                                    if builtin {
                                        "collides with a built-in action"
                                    } else {
                                        "already registered by another plugin"
                                    }
                                ),
                            );
                        }
                        !(builtin || owned_elsewhere)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let contrib = self
            .plugin_contributions
            .entry(name.to_string())
            .or_default();
        contrib.command_ids = commands.iter().map(|c| c.id.clone()).collect();
        self.plugin_manager.register_commands(name, commands);
    }

    pub(crate) fn check_overlay_transitions(&mut self) {
        use super::ActiveOverlays;

        let current = ActiveOverlays {
            help: self.show_help,
            about: self.show_about,
            theme_picker: self.theme_picker.is_some(),
            plugin_picker: self.plugin_picker.is_some(),
            command_palette: self.command_palette.is_some(),
            history: self.history.is_some(),
            repo_log: self.repo_log.is_some(),
            recent_files: self.recent_files.is_some(),
            search: self.search.is_some(),
            in_file_search: self.in_file_search.is_some(),
            tree_filter: self.tree_filter.is_some(),
            bug_report: self.bug_report.is_some(),
            revision_picker: self.revision_picker.is_some(),
            goto_line: self.goto_line.is_some(),
            visual_mode: self.selection.is_some(),
            git_blame: self.show_blame,
        };

        if current.help && !self.active_overlays.help {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::Help,
                });
        }
        if current.about && !self.active_overlays.about {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::About,
                });
        }
        if current.theme_picker && !self.active_overlays.theme_picker {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::ThemePicker,
                });
        }
        if current.plugin_picker && !self.active_overlays.plugin_picker {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::PluginPicker,
                });
        }
        if current.command_palette && !self.active_overlays.command_palette {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::CommandPalette,
                });
        }
        if current.history && !self.active_overlays.history {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::History,
                });
        }
        if current.recent_files && !self.active_overlays.recent_files {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::RecentFiles,
                });
        }
        if current.search && !self.active_overlays.search {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::Search,
                });
        }
        if current.in_file_search && !self.active_overlays.in_file_search {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::InFileSearch,
                });
        }
        if current.tree_filter && !self.active_overlays.tree_filter {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::TreeFilter,
                });
        }
        if current.bug_report && !self.active_overlays.bug_report {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::BugReport,
                });
        }
        if current.revision_picker && !self.active_overlays.revision_picker {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::RevisionPicker,
                });
        }
        if current.goto_line && !self.active_overlays.goto_line {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::OverlayOpened {
                    kind: crate::telemetry::OverlayKind::GotoLine,
                });
        }
        if current.visual_mode && !self.active_overlays.visual_mode {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::FeatureUsed {
                    feature: crate::telemetry::Feature::VisualMode,
                });
        }
        if current.git_blame && !self.active_overlays.git_blame {
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::FeatureUsed {
                    feature: crate::telemetry::Feature::GitBlame,
                });
        }

        self.active_overlays = current;
    }
}

#[cfg(test)]
#[path = "refresh_test.rs"]
mod tests;
