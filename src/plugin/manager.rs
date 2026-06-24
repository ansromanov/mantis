//! Plugin lifecycle manager.
//!
//! [`PluginManager`] owns all registered plugin entries, the running subprocess
//! instances, and any buffered action responses. It provides the public API
//! that `App` calls on file-open, keypress, theme-change, selection-change,
//! and shutdown events.

use std::path::Path;

use crate::plugin::install::default_plugin_dir;
use crate::plugin::process::Plugin;
use crate::plugin::types::{
    Capability, LanguageProviderRegistration, PluginEntry, PluginKind, ToPlugin,
};

/// Manages discovery, lifecycle, and hook dispatch for all plugins.
pub(crate) struct PluginManager {
    entries: Vec<(String, PluginEntry)>,
    plugins: Vec<Plugin>,
    pending_actions: Vec<(String, String, serde_json::Value)>,
    dead_plugins: Vec<String>,
    spawn_errors: Vec<String>,
    active_theme: Option<String>,
    provider_registrations: Vec<LanguageProviderRegistration>,
}

impl PluginManager {
    pub(crate) fn new(entries: Vec<(String, PluginEntry)>) -> Self {
        PluginManager {
            entries,
            plugins: Vec::new(),
            pending_actions: Vec::new(),
            dead_plugins: Vec::new(),
            spawn_errors: Vec::new(),
            active_theme: None,
            provider_registrations: Vec::new(),
        }
    }

    /// Registers a language provider declaration.
    pub(crate) fn register_provider(&mut self, reg: LanguageProviderRegistration) {
        self.provider_registrations
            .retain(|r| r.plugin_name != reg.plugin_name);
        self.provider_registrations.push(reg);
    }

    /// Returns the first registered provider whose extensions include `ext`
    /// (case-insensitive) and whose capabilities include `cap`, if any.
    pub(crate) fn provider_for(
        &self,
        ext: &str,
        cap: &Capability,
    ) -> Option<&LanguageProviderRegistration> {
        let ext_lower = ext.to_ascii_lowercase();
        self.provider_registrations
            .iter()
            .find(|r| r.extensions.iter().any(|e| e == &ext_lower) && r.capabilities.contains(cap))
    }

    /// Spawns all enabled *process* plugins and sends them the `init` event.
    pub(crate) fn activate_all(&mut self, theme_name: Option<&str>) {
        self.active_theme = theme_name.map(|s| s.to_string());
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
                key: None,
                theme: self.active_theme.clone(),
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
                key: None,
                theme: None,
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
                key: None,
                theme: None,
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
                key: Some(key_str.clone()),
                theme: None,
            });
        }
    }

    /// Sends `on_theme_change` to all subscribed active plugins with the new theme name.
    pub(crate) fn on_theme_change(&mut self, theme: &str) {
        self.active_theme = Some(theme.to_string());
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_theme_change") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_theme_change".into(),
                path: None,
                key: None,
                theme: Some(theme.into()),
            });
        }
    }

    /// Sends `on_selection_change` to all subscribed active plugins.
    pub(crate) fn on_selection_change(&mut self, path: Option<&Path>) {
        let path_s = path.map(|p| p.to_string_lossy().into_owned());
        for plugin in &mut self.plugins {
            if !plugin.subscribes_to("on_selection_change") {
                continue;
            }
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: path_s.clone(),
                key: None,
                theme: None,
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
                key: None,
                theme: None,
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
        // Remove dead plugins and track their names.
        self.plugins.retain(|p| !dead.contains(&p.name));
        self.dead_plugins.extend(dead);
    }

    /// Returns the names of plugins detected as dead since the last call.
    /// Dead means the reader channel disconnected (process exited or crashed).
    pub(crate) fn take_dead_plugins(&mut self) -> Vec<String> {
        std::mem::take(&mut self.dead_plugins)
    }

    /// Removes all provider registrations owned by the named plugin.
    pub(crate) fn remove_provider_registrations(&mut self, name: &str) {
        self.provider_registrations
            .retain(|r| r.plugin_name != name);
    }

    /// Consumes and returns all buffered plugin actions since the last call.
    pub(crate) fn take_actions(&mut self) -> Vec<(String, String, serde_json::Value)> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Whether any plugins are currently active.
    pub(crate) fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns every registered plugin as `(name, is_active, kind)`, in order.
    /// For process plugins "active" means a running subprocess; syntax plugins
    /// have no subprocess, so their `enabled` flag stands in (it drives the
    /// palette checkbox and is kept current via [`set_enabled`]).
    pub(crate) fn plugin_entries(&self) -> Vec<(String, bool, PluginKind)> {
        self.entries
            .iter()
            .map(|(name, entry)| {
                let active = if entry.kind == PluginKind::Syntax {
                    entry.enabled
                } else {
                    self.plugins.iter().any(|p| p.name == *name)
                };
                (name.clone(), active, entry.kind.clone())
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
            key: None,
            theme: self.active_theme.clone(),
        });
        if let Some(file) = current_file {
            let path_s = file.to_string_lossy().into_owned();
            plugin.send(&ToPlugin {
                event: "on_file_open".into(),
                path: Some(path_s.clone()),
                key: None,
                theme: None,
            });
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: Some(path_s),
                key: None,
                theme: None,
            });
        }
        self.plugins.push(plugin);
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
            key: None,
            theme: None,
        });
        plugin.close_in_background();
    }
}
