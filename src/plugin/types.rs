//! Plugin types and protocol messages.
//!
//! Shared data structures for the plugin system: process vs syntax plugin kinds,
//! config entries, language provider registrations, and the JSON-line protocol
//! messages exchanged between `mantis` and plugin subprocesses.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What kind of plugin this is.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Standard subprocess plugin (the default).
    #[default]
    Process,
    /// A syntax-definition plugin: provides a `.sublime-syntax` file to extend
    /// the highlighter. No subprocess is spawned.
    Syntax,
}

/// A syntax definition loaded from a plugin, ready to be fed to syntect.
#[derive(Clone, Debug)]
pub struct ExtraSyntax {
    /// Path to the `.sublime-syntax` file on disk.
    pub syntax_path: PathBuf,
    /// File extensions this syntax should match (e.g. `["tf", "tfvars"]`).
    /// May be empty when the syntax definition declares them internally.
    #[allow(dead_code)]
    pub extensions: Vec<String>,
}

/// Per-plugin entry in the `[plugins]` section of `mantis.toml`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PluginEntry {
    /// Path to the plugin executable (process plugins) or syntax file
    /// (syntax plugins). Relative paths are resolved relative to the platform
    /// config directory (see `default_plugin_dir`).
    pub path: PathBuf,
    /// When `false` the plugin is registered but not spawned at startup.
    pub enabled: bool,
    /// Plugin kind. Defaults to `"process"` for backward compatibility.
    pub kind: PluginKind,
    /// File extensions this syntax plugin handles (e.g. `["tf", "tfvars"]`).
    /// Only meaningful when `kind = "syntax"`.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Path to the `.sublime-syntax` file. Only meaningful when
    /// `kind = "syntax"`. Relative paths are resolved against the plugin dir.
    #[serde(default)]
    pub syntax_file: Option<PathBuf>,
    /// Events this plugin subscribes to from the manifest `events` field.
    /// Empty means all events are sent (backward compat).
    #[serde(default)]
    pub events: Vec<String>,
}

impl Default for PluginEntry {
    fn default() -> Self {
        PluginEntry {
            path: PathBuf::new(),
            enabled: true,
            kind: PluginKind::Process,
            extensions: Vec::new(),
            syntax_file: None,
            events: Vec::new(),
        }
    }
}

/// Capabilities a language provider can advertise at `init` time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Syntax highlighting for declared file extensions.
    Highlight,
    /// Code folding regions for declared file extensions.
    Fold,
    /// Hover documentation (reserved; not implemented in 0.8).
    Hover,
    /// Inline diagnostics (reserved; not implemented in 0.8).
    Diagnostics,
    /// Go-to-definition navigation (reserved; not implemented in 0.8).
    Definition,
}

/// A language provider registration received from a plugin via the
/// `register_language_provider` action after `init`.
///
/// The host stores one registration per plugin-declaration and uses it to
/// route capabilities to the correct provider when a file is opened.
#[derive(Clone, Debug)]
pub struct LanguageProviderRegistration {
    /// Name of the plugin that sent this registration.
    pub plugin_name: String,
    /// Lowercase file extensions handled by this provider (no leading dot).
    pub extensions: Vec<String>,
    /// Capabilities declared by this provider.
    pub capabilities: std::collections::HashSet<Capability>,
}

/// Message sent from `mantis` to a plugin (on its stdin).
#[derive(Serialize)]
pub(crate) struct ToPlugin {
    pub(crate) event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) theme: Option<String>,
    /// Protocol version spoken by the host. Present only on the `init` event
    /// so the plugin can verify compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) protocol_version: Option<String>,
}

/// Message received from a plugin (on its stdout).
#[derive(Deserialize)]
pub(crate) struct FromPlugin {
    #[allow(dead_code)]
    pub(crate) event: String,
    pub(crate) action: Option<String>,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
}

/// Tracks what application state a plugin has contributed so that disabling
/// or crashing the plugin tears down exactly its output without affecting
/// other plugins' state. One entry per running plugin.
///
/// Every `set_*` action handler in `App::handle_plugin_action` must stamp
/// the originating plugin's contribution here. The teardown method
/// (`App::teardown_plugin_contributions`) reads this map to know which
/// fields to clear, replacing the former per-plugin-name special cases
/// (e.g. the old `if name == "iconize"` branch).
#[derive(Clone, Debug, Default)]
pub(crate) struct PluginContributions {
    /// Paths in `plugin_content` / `plugin_content_text` rendered by this plugin.
    pub(crate) content_paths: HashSet<PathBuf>,
    /// Paths in `plugin_fold_regions` registered by this plugin.
    pub(crate) fold_region_paths: HashSet<PathBuf>,
    /// Whether this plugin set the icon map / icon fields via `set_icon_map`.
    pub(crate) has_icon_map: bool,
}
