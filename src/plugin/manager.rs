//! Plugin lifecycle manager.
//!
//! [`PluginManager`] owns all registered plugin entries, the running subprocess
//! instances, and any buffered action responses. It provides the public API
//! that `App` calls on file-open, keypress, theme-change, selection-change,
//! debounced content-cursor, and shutdown events.
//!
//! Provider routing checks exact filename, filename glob, extension, and
//! cached shebang matches in that order, using registration priority only for
//! matches of the same specificity. [`PluginManager::send_request`]/[`poll_requests`]
//! implement the host side of the `request`/`response` correlation (see
//! `crate::plugin::process`), tracking outstanding requests in
//! `pending_requests` and timing them out after [`REQUEST_TIMEOUT`] without
//! killing the plugin; diagnostics timeouts stay quiet because they are an
//! optional, read-only refresh. [`provider_for_file`] resolves the winning
//! capability provider, and [`register_provider`] raises a one-time warning when two
//! plugins claim the same rule and capability. A `plugin_error` action
//! (reported via [`record_plugin_error`]) is tracked in `last_plugin_error`,
//! a struct parallel to the existing crash-diagnostics `last_crash` map, so
//! it can surface in the plugin picker's badge without marking the plugin
//! dead.
//!
//! [`provider_for_file`]: PluginManager::provider_for_file
//! [`register_provider`]: PluginManager::register_provider
//! [`record_plugin_error`]: PluginManager::record_plugin_error
//! [`poll_requests`]: PluginManager::poll_requests

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::plugin::install::default_plugin_dir;
use crate::plugin::process::Plugin;
use crate::plugin::types::{
    Capability, LanguageProviderRegistration, PluginCommand, PluginContextItem, PluginEntry,
    PluginKind, PluginStatusSegment, ThemeColorsMsg, ToPlugin,
};
use crate::theme::Theme;

/// Diagnostics captured from a plugin's stderr at the moment it was found dead.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CrashInfo {
    pub(crate) last_stderr: Option<String>,
    pub(crate) log_path: Option<PathBuf>,
}

/// Diagnostics captured from a `plugin_error` action (protocol 3+). Parallel
/// to [`CrashInfo`], but for a *live* plugin reporting a soft failure rather
/// than a dead one — it never marks the plugin as not running.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PluginErrorInfo {
    pub(crate) message: String,
    pub(crate) context: Option<String>,
}

/// A `request` awaiting its correlated `response` (protocol 3+).
pub(super) struct PendingRequest {
    pub(super) plugin_name: String,
    pub(super) method: String,
    pub(super) deadline: Instant,
}

/// How long the host waits for a plugin's `response` before treating the
/// request as timed out (surfaced the same way as `plugin_error`, without
/// killing the plugin). Lengthened under `cfg(test)` so a real spawned
/// subprocess's round trip (fork/exec + pipe I/O) under parallel
/// test-suite load isn't racing a razor-thin window.
#[cfg(not(test))]
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_millis(300);
#[cfg(test)]
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Manages discovery, lifecycle, and hook dispatch for all plugins.
pub(crate) struct PluginManager {
    entries: Vec<(String, PluginEntry)>,
    pub(crate) plugins: Vec<Plugin>,
    pending_actions: Vec<(String, String, serde_json::Value)>,
    dead_plugins: Vec<String>,
    /// Diagnostics for the most recent crash of each plugin, keyed by name.
    /// Cleared on a successful manual restart via `activate_one`.
    last_crash: HashMap<String, CrashInfo>,
    /// Diagnostics for the most recent `plugin_error` action or request
    /// timeout for each (still-running) plugin, keyed by name. Cleared on a
    /// successful manual restart via `activate_one`.
    last_plugin_error: HashMap<String, PluginErrorInfo>,
    spawn_errors: Vec<String>,
    active_theme: Option<String>,
    active_theme_colors: Option<ThemeColorsMsg>,
    provider_registrations: Vec<LanguageProviderRegistration>,
    /// (match rule, capability) pairs that already produced a conflict warning.
    provider_conflicts_warned: HashSet<(String, Capability)>,
    /// Shebang interpreter cached by file load, keyed by absolute path.
    file_shebangs: HashMap<PathBuf, String>,
    /// Counter for allocating `request` ids, incremented per outstanding
    /// request across all plugins. Never reused while a request with that id
    /// is outstanding.
    next_request_id: u64,
    /// Requests sent via `send_request` awaiting a `response`, keyed by id.
    pub(super) pending_requests: HashMap<u64, PendingRequest>,
    request_spans: HashMap<u64, tracing::Span>,
    /// Plugin-contributed palette commands, keyed by plugin name. A
    /// `BTreeMap` so palette ordering across plugins is deterministic.
    command_registrations: BTreeMap<String, Vec<PluginCommand>>,
    /// Plugin-contributed context-menu items, keyed by plugin name.
    context_item_registrations: BTreeMap<String, Vec<PluginContextItem>>,
    /// Plugin-contributed status segments, keyed by plugin name.
    status_segment_registrations: BTreeMap<String, Vec<PluginStatusSegment>>,
}

impl PluginManager {
    pub(crate) fn new(entries: Vec<(String, PluginEntry)>) -> Self {
        PluginManager {
            entries,
            plugins: Vec::new(),
            pending_actions: Vec::new(),
            dead_plugins: Vec::new(),
            last_crash: HashMap::new(),
            last_plugin_error: HashMap::new(),
            spawn_errors: Vec::new(),
            active_theme: None,
            active_theme_colors: None,
            provider_registrations: Vec::new(),
            provider_conflicts_warned: HashSet::new(),
            file_shebangs: HashMap::new(),
            next_request_id: 0,
            pending_requests: HashMap::new(),
            request_spans: HashMap::new(),
            command_registrations: BTreeMap::new(),
            context_item_registrations: BTreeMap::new(),
            status_segment_registrations: BTreeMap::new(),
        }
    }

    /// Registers a language provider declaration. Returns a one-time
    /// status-bar warning string the first time a matching rule and capability
    /// conflict with another plugin's registration — `None` otherwise
    /// (including on every later registration of an already-warned pair).
    pub(crate) fn register_provider(
        &mut self,
        reg: LanguageProviderRegistration,
    ) -> Option<String> {
        self.provider_registrations
            .retain(|r| r.plugin_name != reg.plugin_name);

        let mut warning = None;
        'outer: for (rule, label) in provider_rules(&reg) {
            for cap in &reg.capabilities {
                let warning_key = (rule.clone(), cap.clone());
                if self.provider_conflicts_warned.contains(&warning_key) {
                    continue;
                }
                let Some(existing) = self.provider_registrations.iter().find(|existing| {
                    existing.plugin_name != reg.plugin_name
                        && existing.capabilities.contains(cap)
                        && provider_rules(existing)
                            .iter()
                            .any(|(existing_rule, _)| existing_rule == &rule)
                }) else {
                    continue;
                };
                self.provider_conflicts_warned.insert(warning_key);
                warning = Some(format!(
                    "Plugins '{}' and '{}' both register '{}' for {label}; higher priority wins",
                    existing.plugin_name,
                    reg.plugin_name,
                    capability_label(cap),
                ));
                break 'outer;
            }
        }

        self.provider_registrations.push(reg);
        warning
    }

    /// Returns the registered provider whose extensions include `ext`
    /// (case-insensitive) and whose capabilities include `cap`, breaking ties
    /// between multiple matches by highest `priority`; equal priority keeps
    /// whichever was registered first (earliest in registration order).
    #[cfg(test)]
    pub(crate) fn provider_for(
        &self,
        ext: &str,
        cap: &Capability,
    ) -> Option<&LanguageProviderRegistration> {
        let ext_lower = ext.to_ascii_lowercase();
        let mut best: Option<&LanguageProviderRegistration> = None;
        for reg in self.provider_registrations.iter().filter(|r| {
            r.extensions.iter().any(|e| e == &ext_lower) && r.capabilities.contains(cap)
        }) {
            if best.is_none_or(|cur| reg.priority > cur.priority) {
                best = Some(reg);
            }
        }
        best
    }

    /// Caches or clears the loader-detected interpreter for a file path.
    pub(crate) fn cache_file_shebang(&mut self, path: &Path, shebang: Option<String>) {
        if let Some(interpreter) = shebang {
            self.file_shebangs
                .insert(path.to_path_buf(), interpreter.to_ascii_lowercase());
        } else {
            self.file_shebangs.remove(path);
        }
    }

    /// Returns the most specific provider for `path` and `cap`. Match
    /// specificity precedes priority: exact filename, glob, extension, then
    /// shebang. Registration order breaks equal-priority ties.
    pub(crate) fn provider_for_file(
        &self,
        path: &Path,
        cap: &Capability,
    ) -> Option<&LanguageProviderRegistration> {
        let file_name = path.file_name()?.to_str().unwrap_or_default();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        let shebang = self.file_shebangs.get(path).map(String::as_str);
        let mut best: Option<(&LanguageProviderRegistration, u8)> = None;
        for reg in self.provider_registrations.iter().filter(|reg| {
            reg.capabilities.contains(cap)
                && (reg.filenames.iter().any(|name| name == file_name)
                    || reg
                        .filenames
                        .iter()
                        .any(|pattern| glob_matches(pattern, file_name))
                    || reg
                        .extensions
                        .iter()
                        .any(|candidate| !ext.is_empty() && candidate.eq_ignore_ascii_case(ext))
                    || shebang.is_some_and(|interpreter| {
                        reg.shebangs
                            .iter()
                            .any(|candidate| candidate.eq_ignore_ascii_case(interpreter))
                    }))
        }) {
            let specificity = if reg.filenames.iter().any(|name| name == file_name) {
                4
            } else if reg
                .filenames
                .iter()
                .any(|pattern| glob_matches(pattern, file_name))
            {
                3
            } else if reg
                .extensions
                .iter()
                .any(|candidate| !ext.is_empty() && candidate.eq_ignore_ascii_case(ext))
            {
                2
            } else {
                1
            };
            if best.is_none_or(|(current, rank)| {
                specificity > rank || (specificity == rank && reg.priority > current.priority)
            }) {
                best = Some((reg, specificity));
            }
        }
        best.map(|(reg, _)| reg)
    }

    /// Sends a `request` to the named running plugin, allocating a fresh id
    /// and recording it as pending with a [`REQUEST_TIMEOUT`] deadline.
    /// Returns the allocated id, or `None` if no plugin with that name is
    /// currently running.
    pub(crate) fn send_request(
        &mut self,
        plugin_name: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Option<u64> {
        let plugin = self.plugins.iter_mut().find(|p| p.name == plugin_name)?;
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.wrapping_add(1);
        let span = tracing::info_span!("plugin_round_trip");
        self.request_spans.insert(id, span);
        plugin.send(&ToPlugin {
            event: "request".into(),
            path: None,
            line: None,
            column: None,
            key: None,
            theme: None,
            colors: None,
            protocol_version: None,
            id: Some(id),
            method: Some(method.to_string()),
            params: Some(params),
        });
        self.pending_requests.insert(
            id,
            PendingRequest {
                plugin_name: plugin_name.to_string(),
                method: method.to_string(),
                deadline: Instant::now() + REQUEST_TIMEOUT,
            },
        );
        Some(id)
    }

    /// Called once per tick to collect completed request/response pairs:
    /// drains every plugin's buffered `response` messages matched by id, and
    /// treats any pending request whose deadline has passed as a timeout
    /// error, recording it exactly like a `plugin_error` (see
    /// `record_plugin_error`) without killing the plugin. Companion to
    /// `drain_actions`/`take_actions`.
    pub(crate) fn poll_requests(&mut self) -> Vec<(u64, Result<serde_json::Value, String>)> {
        let mut results = Vec::new();
        for plugin in &mut self.plugins {
            for (id, result) in plugin.drain_responses() {
                if self.pending_requests.remove(&id).is_some() {
                    results.push((id, result));
                    self.request_spans.remove(&id);
                }
                // Unknown or already-timed-out id: silently dropped.
            }
        }
        let now = Instant::now();
        let expired: Vec<u64> = self
            .pending_requests
            .iter()
            .filter(|(_, pending)| pending.deadline <= now)
            .map(|(id, _)| *id)
            .collect();
        for id in expired {
            let Some(pending) = self.pending_requests.remove(&id) else {
                continue;
            };
            self.request_spans.remove(&id);
            let message = format!("request {id} to plugin '{}' timed out", pending.plugin_name);
            if pending.method != "diagnostics" {
                self.record_plugin_error(
                    &pending.plugin_name,
                    message.clone(),
                    Some("request".into()),
                );
            }
            results.push((id, Err(message)));
        }
        results
    }

    /// Records a `plugin_error` action (or request timeout) for `name`,
    /// replacing any previous entry, so the plugin picker can badge it and
    /// the status bar can surface it — without marking the plugin dead.
    pub(crate) fn record_plugin_error(
        &mut self,
        name: &str,
        message: String,
        context: Option<String>,
    ) {
        self.last_plugin_error
            .insert(name.to_string(), PluginErrorInfo { message, context });
    }

    /// Returns the most recently recorded `plugin_error` diagnostics for
    /// `name`, if any.
    #[allow(dead_code)]
    pub(crate) fn plugin_error_for(&self, name: &str) -> Option<&PluginErrorInfo> {
        self.last_plugin_error.get(name)
    }

    /// Appends `line` to the named plugin's rotating stderr log, if it has
    /// one. Used to record a `plugin_error` action's message in the same
    /// on-disk diagnostics as crash output.
    pub(crate) fn log_plugin_error_line(&self, name: &str, line: &str) {
        if let Some(plugin) = self.plugins.iter().find(|p| p.name == name) {
            if let Some(path) = plugin.log_path() {
                crate::plugin::process::append_plugin_log_line(&path, line);
            }
        }
    }

    /// Returns `true` if any currently running plugin *explicitly*
    /// subscribes to `on_keypress` (see `Plugin::wants_key_consumption`), so
    /// the caller can decide whether to defer normal-mode key handling for a
    /// chance at `key_handled`. Plugins that receive `on_keypress` only via
    /// the empty-`events` back-compat wildcard do not count — they never
    /// asked to gate input and would otherwise delay every keystroke.
    pub(crate) fn has_keypress_subscriber(&self) -> bool {
        self.plugins.iter().any(|p| p.wants_key_consumption())
    }

    /// Spawns all enabled *process* plugins and sends them the `init` event.
    /// Ensures bundled plugins are installed on disk before spawning.
    pub(crate) fn activate_all(&mut self, theme_name: Option<&str>, theme: &Theme) {
        self.active_theme = theme_name.map(|s| s.to_string());
        self.active_theme_colors = Some(ThemeColorsMsg::from(theme));
        crate::plugin::install_bundled_plugins();
        let plugin_dir = default_plugin_dir();
        for (name, entry) in &self.entries {
            if !entry.enabled || entry.kind != PluginKind::Process {
                continue;
            }
            let path = if entry.path.is_relative() {
                plugin_dir.join(&entry.path)
            } else {
                entry.path.clone()
            };
            let events = entry.events.clone();
            let mut plugin = Plugin::new(name.clone(), events);
            if let Err(e) = plugin.spawn(&path) {
                self.spawn_errors.push(e);
                continue;
            }
            plugin.send(&ToPlugin {
                event: "init".into(),
                path: None,
                line: None,
                column: None,
                key: None,
                theme: self.active_theme.clone(),
                colors: self.active_theme_colors.clone(),
                protocol_version: Some(crate::plugin::PROTOCOL_VERSION.into()),
                id: None,
                method: None,
                params: None,
            });
            self.plugins.push(plugin);
        }
    }

    /// Returns (and clears) any errors that occurred while spawning plugins
    /// during `activate_all`.
    pub(crate) fn take_spawn_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.spawn_errors)
    }

    /// Sends `shutdown` to all plugins, then closes each subprocess.
    #[allow(dead_code)]
    pub(crate) fn deactivate_all(&mut self) {
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "shutdown".into(),
                path: None,
                line: None,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
        for mut plugin in self.plugins.drain(..) {
            plugin.close();
        }
    }

    /// Sends `on_file_open` to all subscribed active plugins.
    pub(crate) fn on_file_open(&mut self, path: &Path) {
        let path_s = path.to_string_lossy().into_owned();
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_file_open") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_file_open".into(),
                path: Some(path_s.clone()),
                line: None,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Sends `on_keypress` to all subscribed active plugins with a human-readable key.
    pub(crate) fn on_keypress(&mut self, key: &crossterm::event::KeyEvent) {
        let key_str = super::key_event_to_string(key);
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_keypress") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_keypress".into(),
                path: None,
                line: None,
                column: None,
                key: Some(key_str.clone()),
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Sends `on_theme_change` to all subscribed active plugins with the new
    /// theme name and its resolved colors.
    pub(crate) fn on_theme_change(&mut self, theme_name: &str, theme: &Theme) {
        self.active_theme = Some(theme_name.to_string());
        self.active_theme_colors = Some(ThemeColorsMsg::from(theme));
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_theme_change") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_theme_change".into(),
                path: None,
                line: None,
                column: None,
                key: None,
                theme: Some(theme_name.into()),
                colors: self.active_theme_colors.clone(),
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Sends a path-only `on_selection_change` to all subscribed active plugins.
    pub(crate) fn on_selection_change(&mut self, path: Option<&Path>) {
        self.on_selection_change_at(path, None);
    }

    /// Sends `on_selection_change` with an optional zero-based physical line.
    /// The line is additive, so existing plugins can continue to consume only
    /// the path while language-aware plugins can track the active document.
    pub(crate) fn on_selection_change_at(&mut self, path: Option<&Path>, line: Option<usize>) {
        let path_s = path.map(|p| p.to_string_lossy().into_owned());
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_selection_change") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: path_s.clone(),
                line,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Sends a content-cursor position to plugins that subscribe to
    /// `on_content_cursor_change`. Coordinates are one-based for plugin use.
    pub(crate) fn on_content_cursor_change(&mut self, path: &Path, line: usize, column: usize) {
        let path = path.to_string_lossy().into_owned();
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_content_cursor_change") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_content_cursor_change".into(),
                path: Some(path.clone()),
                line: Some(line),
                column: Some(column),
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Sends `on_quit` to all subscribed active plugins (graceful shutdown notice).
    #[allow(dead_code)]
    pub(crate) fn on_quit(&mut self) {
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_quit") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_quit".into(),
                path: None,
                line: None,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
    }

    /// Non-blockingly drains pending actions from every plugin's reader channel
    /// into an internal buffer. Call `take_actions` to collect them.
    ///
    /// Plugins whose reader channel has disconnected (process exited / crashed)
    /// are removed from `self.plugins` and their names are added to
    /// `dead_plugins` so the caller can tear down their contributions.
    pub(crate) fn drain_actions(&mut self) {
        let mut dead = Vec::new();
        for plugin in &mut self.plugins {
            let (actions, is_dead) = plugin.drain_actions();
            if is_dead {
                dead.push(plugin.name.clone());
            }
            for (action, params) in actions {
                self.pending_actions
                    .push((plugin.name.clone(), action, params));
            }
        }
        // Capture stderr diagnostics before the dead plugins are dropped.
        for name in &dead {
            if let Some(plugin) = self.plugins.iter().find(|p| &p.name == name) {
                self.last_crash.insert(
                    name.clone(),
                    CrashInfo {
                        last_stderr: plugin.last_stderr_line(),
                        log_path: plugin.log_path().filter(|p| p.exists()),
                    },
                );
            }
        }
        // Remove dead plugins and track their names.
        self.plugins.retain(|p| !dead.contains(&p.name));
        self.dead_plugins.extend(dead);
    }

    /// Returns the names of plugins detected as dead since the last call.
    /// Dead means the reader channel disconnected (process exited or crashed).
    pub(crate) fn take_dead_plugins(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dead_plugins)
    }

    /// Returns the captured stderr diagnostics for the most recent crash of
    /// `name`, if any was recorded.
    pub(crate) fn crash_detail(&self, name: &str) -> Option<&CrashInfo> {
        self.last_crash.get(name)
    }

    /// Removes all provider registrations owned by the named plugin.
    pub(crate) fn remove_provider_registrations(&mut self, name: &str) {
        self.provider_registrations
            .retain(|r| r.plugin_name != name);
    }

    /// Register palette commands from a plugin. Replaces any prior
    /// registration from the same plugin (idempotent re-registration).
    pub(crate) fn register_commands(&mut self, plugin_name: &str, commands: Vec<PluginCommand>) {
        if commands.is_empty() {
            self.command_registrations.remove(plugin_name);
        } else {
            self.command_registrations
                .insert(plugin_name.to_string(), commands);
        }
    }

    /// Remove all command registrations for a named plugin (teardown).
    pub(crate) fn remove_command_registrations(&mut self, name: &str) {
        self.command_registrations.remove(name);
    }

    /// Which plugin owns a given command id, if any.
    pub(crate) fn plugin_for_command(&self, command_id: &str) -> Option<&str> {
        for (name, cmds) in &self.command_registrations {
            if cmds.iter().any(|c| c.id == command_id) {
                return Some(name.as_str());
            }
        }
        None
    }

    /// All registered plugin commands (for palette construction).
    pub(crate) fn all_plugin_commands(&self) -> Vec<&PluginCommand> {
        self.command_registrations.values().flatten().collect()
    }

    /// Send a `command` event to the plugin that owns the given command id.
    pub(crate) fn send_command_event(&mut self, command_id: &str) {
        let Some(plugin_name) = self.plugin_for_command(command_id).map(str::to_string) else {
            return;
        };
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == plugin_name) {
            plugin.send(&ToPlugin {
                event: "command".into(),
                path: None,
                line: None,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: Some(serde_json::json!({"id": command_id})),
            });
        }
    }

    /// Registers context-menu items contributed by `plugin_name`.
    pub(crate) fn register_context_items(
        &mut self,
        plugin_name: &str,
        items: Vec<PluginContextItem>,
    ) {
        if items.is_empty() {
            self.context_item_registrations.remove(plugin_name);
        } else {
            self.context_item_registrations
                .insert(plugin_name.to_string(), items);
        }
    }

    /// Removes context-menu items registered by `plugin_name`.
    pub(crate) fn remove_context_items(&mut self, plugin_name: &str) {
        self.context_item_registrations.remove(plugin_name);
    }

    /// All registered plugin context items as `(plugin_name, item)`.
    pub(crate) fn all_context_items(&self) -> Vec<(&str, &PluginContextItem)> {
        self.context_item_registrations
            .iter()
            .flat_map(|(name, items)| items.iter().map(move |item| (name.as_str(), item)))
            .collect()
    }

    /// Registers status segments contributed by `plugin_name`.
    pub(crate) fn register_status_segments(
        &mut self,
        plugin_name: &str,
        segments: Vec<PluginStatusSegment>,
    ) {
        if segments.is_empty() {
            self.status_segment_registrations.remove(plugin_name);
        } else {
            self.status_segment_registrations
                .insert(plugin_name.to_string(), segments);
        }
    }

    /// Removes status segments registered by `plugin_name`.
    pub(crate) fn remove_status_segments(&mut self, plugin_name: &str) {
        self.status_segment_registrations.remove(plugin_name);
    }

    /// All registered plugin status segments as `(plugin_name, segment)`.
    pub(crate) fn all_status_segments(&self) -> Vec<(&str, &PluginStatusSegment)> {
        self.status_segment_registrations
            .iter()
            .flat_map(|(name, segs)| segs.iter().map(move |seg| (name.as_str(), seg)))
            .collect()
    }

    /// Send a context menu `command` event to the plugin that owns the item.
    pub(crate) fn send_context_menu_event(
        &mut self,
        plugin_name: &str,
        item_id: &str,
        target_kind: &str,
        path: Option<String>,
        line: Option<usize>,
    ) {
        if let Some(plugin) = self.plugins.iter_mut().find(|p| p.name == plugin_name) {
            plugin.send(&ToPlugin {
                event: "command".into(),
                path,
                line,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: Some(serde_json::json!({
                    "id": item_id,
                    "target": target_kind,
                })),
            });
        }
    }

    /// Consumes and returns all buffered plugin actions since the last call.
    pub(crate) fn take_actions(&mut self) -> Vec<(String, String, serde_json::Value)> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Whether any plugins are currently active.
    pub(crate) fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns `true` if a plugin with the given name is currently active.
    pub(crate) fn is_plugin_active(&self, name: &str) -> bool {
        self.plugins.iter().any(|p| p.name == name)
    }

    /// Returns every registered plugin as `(name, is_active, kind, crash_badge)`,
    /// in order. For process plugins "active" means a running subprocess;
    /// syntax plugins have no subprocess, so their `enabled` flag stands in
    /// (it drives the palette checkbox and is kept current via
    /// [`set_enabled`]). `crash_badge` is a short diagnostic summary: either
    /// why the plugin isn't running (it last exited unexpectedly), or — for
    /// a still-running plugin — the most recent `plugin_error` it reported.
    pub(crate) fn plugin_entries(&self) -> Vec<(String, bool, PluginKind, Option<String>)> {
        self.entries
            .iter()
            .map(|(name, entry)| {
                let active = if entry.kind == PluginKind::Syntax {
                    entry.enabled
                } else {
                    self.plugins.iter().any(|p| p.name == *name)
                };
                let crash_badge = if !active {
                    self.last_crash.get(name).map(crash_summary)
                } else {
                    self.last_plugin_error.get(name).map(plugin_error_summary)
                };
                (name.clone(), active, entry.kind.clone(), crash_badge)
            })
            .collect()
    }

    /// Updates the stored `enabled` flag for a registered plugin. Used for
    /// syntax plugins, whose enabled state (not a subprocess) drives the
    /// palette checkbox returned by [`plugin_entries`].
    pub(crate) fn set_enabled(&mut self, name: &str, enabled: bool) {
        if let Some((_, entry)) = self.entries.iter_mut().find(|(n, _)| n == name) {
            entry.enabled = enabled;
        }
    }

    /// Spawns a single registered plugin by name, sends it `init`, and
    /// optionally follows up with `on_file_open` + `on_selection_change`.
    /// No-op if already running. Syntax-kind plugins are rejected (they have
    /// no subprocess to spawn).
    pub(crate) fn activate_one(
        &mut self,
        name: &str,
        current_file: Option<&Path>,
        current_line: Option<usize>,
    ) -> Result<(), String> {
        if self.plugins.iter().any(|p| p.name == name) {
            return Ok(());
        }
        let entry = self
            .entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, e)| e.clone())
            .ok_or_else(|| format!("plugin '{name}' not registered"))?;
        if entry.kind != PluginKind::Process {
            return Err(format!(
                "cannot activate a non-process plugin ('{name}') as a subprocess"
            ));
        }
        let plugin_dir = default_plugin_dir();
        let path = if entry.path.is_relative() {
            plugin_dir.join(&entry.path)
        } else {
            entry.path.clone()
        };
        let events = entry.events.clone();
        let mut plugin = Plugin::new(name.to_string(), events);
        plugin.spawn(&path)?;
        plugin.send(&ToPlugin {
            event: "init".into(),
            path: None,
            line: None,
            column: None,
            key: None,
            theme: self.active_theme.clone(),
            colors: self.active_theme_colors.clone(),
            protocol_version: Some(crate::plugin::PROTOCOL_VERSION.into()),
            id: None,
            method: None,
            params: None,
        });
        if let Some(file) = current_file {
            let path_s = file.to_string_lossy().into_owned();
            plugin.send(&ToPlugin {
                event: "on_file_open".into(),
                path: Some(path_s.clone()),
                line: None,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: Some(path_s),
                line: current_line,
                column: None,
                key: None,
                theme: None,
                colors: None,
                protocol_version: None,
                id: None,
                method: None,
                params: None,
            });
        }
        self.plugins.push(plugin);
        self.last_crash.remove(name);
        self.last_plugin_error.remove(name);
        Ok(())
    }

    /// Sends `shutdown` to a single running plugin and closes its subprocess.
    /// No-op if no plugin with that name is running.
    pub(crate) fn deactivate_one(&mut self, name: &str) {
        let Some(pos) = self.plugins.iter().position(|p| p.name == name) else {
            return;
        };
        let mut plugin = self.plugins.remove(pos);
        plugin.send(&ToPlugin {
            event: "shutdown".into(),
            path: None,
            line: None,
            column: None,
            key: None,
            theme: None,
            colors: None,
            protocol_version: None,
            id: None,
            method: None,
            params: None,
        });
        plugin.close_in_background();
    }
}

/// Renders a `CrashInfo` into the short summary shown next to a dead
/// plugin's entry in the plugin picker (a `!` badge).
fn crash_summary(info: &CrashInfo) -> String {
    match (&info.last_stderr, &info.log_path) {
        (Some(line), Some(path)) => format!("{line} (log: {})", path.display()),
        (Some(line), None) => line.clone(),
        (None, Some(path)) => format!("log: {}", path.display()),
        (None, None) => "exited unexpectedly".to_string(),
    }
}

/// Renders a `PluginErrorInfo` into the short summary shown next to a
/// still-running plugin's entry in the plugin picker (a `!` badge), parallel
/// to `crash_summary` for dead plugins.
fn plugin_error_summary(info: &PluginErrorInfo) -> String {
    match &info.context {
        Some(context) => format!("{} ({context})", info.message),
        None => info.message.clone(),
    }
}

/// Returns stable identities and display labels for a provider's conflict
/// rules. Prefixes keep identical text in different match kinds distinct.
fn provider_rules(reg: &LanguageProviderRegistration) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    for filename in &reg.filenames {
        let kind = if filename.contains('*') || filename.contains('?') {
            "glob"
        } else {
            "filename"
        };
        rules.push((
            format!("{kind}:{filename}"),
            format!("filename pattern '{filename}'"),
        ));
    }
    rules.extend(reg.extensions.iter().map(|ext| {
        let ext = ext.to_ascii_lowercase();
        (format!("extension:{ext}"), format!(".{ext}"))
    }));
    rules.extend(reg.shebangs.iter().map(|interpreter| {
        let interpreter = interpreter.to_ascii_lowercase();
        (
            format!("shebang:{interpreter}"),
            format!("shebang '{interpreter}'"),
        )
    }));
    rules
}

/// Matches a filename against the `*` and `?` wildcards accepted in provider
/// filename patterns. Matching is case-sensitive, like exact filename rules.
fn glob_matches(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let value: Vec<char> = value.chars().collect();
    let mut rows = vec![vec![false; value.len() + 1]; pattern.len() + 1];
    rows[0][0] = true;
    for (i, token) in pattern.iter().enumerate() {
        match token {
            '*' => {
                rows[i + 1][0] = rows[i][0];
                for j in 1..=value.len() {
                    rows[i + 1][j] = rows[i][j] || rows[i + 1][j - 1];
                }
            }
            '?' => {
                for j in 1..=value.len() {
                    rows[i + 1][j] = rows[i][j - 1];
                }
            }
            literal => {
                for j in 1..=value.len() {
                    rows[i + 1][j] = rows[i][j - 1] && *literal == value[j - 1];
                }
            }
        }
    }
    rows[pattern.len()][value.len()]
}

/// Human-readable label for a capability, used in the provider-conflict
/// status-bar warning.
fn capability_label(cap: &Capability) -> &'static str {
    match cap {
        Capability::Highlight => "highlight",
        Capability::Fold => "fold",
        Capability::Hover => "hover",
        Capability::Diagnostics => "diagnostics",
        Capability::Definition => "definition",
        Capability::StatusFacts => "status_facts",
        Capability::Symbols => "symbols",
    }
}
