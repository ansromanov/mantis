//! Plugin lifecycle methods on `App`: toggle, activate, deactivate, and
//! rebuild syntax definitions.
//!
//! Extracted from `mod.rs` to stay under the 700-line limit.

use crate::plugin;

use super::App;

impl App {
    /// Toggles the currently highlighted plugin in the picker: spawns it if
    /// stopped, kills it if running, or flips the enabled flag for syntax
    /// plugins and reloads syntax definitions so the change takes effect
    /// immediately. Updates `config.plugins[name].enabled` and writes `mantis.toml`
    /// so the change persists across restarts.
    pub(crate) fn toggle_plugin_picker_selection(&mut self) {
        let Some(picker) = &self.plugin_picker else {
            return;
        };
        let Some((name, running, kind)) = picker.entries.get(picker.selected).cloned() else {
            return;
        };
        if kind == plugin::PluginKind::Syntax {
            // Syntax plugin: just flip the enabled flag and rebuild syntaxes.
            let was_enabled = self
                .config
                .plugins
                .get(&name)
                .map(|e| e.enabled)
                .unwrap_or(false);
            if let Some(entry) = self.config.plugins.get_mut(&name) {
                entry.enabled = !was_enabled;
            }
            self.plugin_manager.set_enabled(&name, !was_enabled);
            self.save_config();
            self.rebuild_extra_syntaxes();
            self.reload_content();
        } else if running {
            self.plugin_manager.deactivate_one(&name);
            if let Some(entry) = self.config.plugins.get_mut(&name) {
                entry.enabled = false;
            }
            self.save_config();
            // Tear down all state this plugin produced, then re-render the
            // current file without plugin content. This replaces the former
            // per-plugin-name special case (e.g. `if name == "iconize"`).
            self.teardown_plugin_contributions(&name);
        } else {
            // Ensure the plugin file is present on disk before spawning.
            plugin::install_bundled_plugins();
            match self
                .plugin_manager
                .activate_one(&name, self.current_file.as_deref())
            {
                Ok(()) => {
                    if let Some(entry) = self.config.plugins.get_mut(&name) {
                        entry.enabled = true;
                    }
                    self.save_config();
                }
                Err(e) => {
                    self.plugin_message = Some(format!("Plugin error: {e}"));
                }
            }
        }
        let updated = self.plugin_manager.plugin_entries();
        if let Some(picker) = &mut self.plugin_picker {
            picker.entries = updated;
        }
    }

    /// Rebuilds the `extra_syntaxes` list from the current config and updates
    /// the main-thread highlighter and the worker thread's highlighter so that
    /// syntax highlighting reflects the latest set of enabled syntax plugins.
    fn rebuild_extra_syntaxes(&mut self) {
        let mut plugin_entries: Vec<_> = self.config.plugins.clone().into_iter().collect();
        plugin_entries.sort_by(|a, b| a.0.cmp(&b.0));
        self.extra_syntaxes = plugin::load_extra_syntaxes(&plugin_entries);
        self.highlighter = crate::highlight::Highlighter::with_extra_syntaxes(
            &self.theme.syntax,
            &self.extra_syntaxes,
        );
        self.loader_set_extra_syntaxes();
    }
}
