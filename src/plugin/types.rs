//! Plugin types and protocol messages.
//!
//! Shared data structures for the plugin system: process vs syntax plugin kinds,
//! config entries, language provider registrations, and the JSON-line protocol
//! messages exchanged between `mantis` and plugin subprocesses.
//!
//! Protocol 3 additions live here too: [`ToPlugin`] gained `id`/`method`/`params`
//! fields so it can also carry a `request` event (host → plugin, answered by a
//! correlated `response` on stdout — see `crate::plugin::process`), and
//! [`FromPlugin`] gained `id`/`result`/`error` so the reader thread can parse
//! those `response` lines. [`LanguageProviderRegistration`] selects providers
//! by exact filename, filename glob, extension, or cached shebang, then uses
//! priority to break ties between registrations of equal specificity. The
//! `symbols` capability and [`PluginContributions::symbol_paths`] extend the
//! push-based language-provider lifecycle with per-file symbol lists that are
//! removed when their provider exits.

use std::collections::HashSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::{color_to_hex, Theme};

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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
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
    /// Free-text status-bar facts for declared file extensions, delivered via
    /// `set_status_facts` (protocol 3+). Parallel to YAML's built-in
    /// anchor/alias counts, but generic: any provider can contribute a short
    /// summary string (e.g. resource identity, per-kind counts) without the
    /// host special-casing a language.
    StatusFacts,
    /// Flat symbol outlines for declared file extensions, delivered via
    /// `set_symbols`.
    Symbols,
}

/// One named symbol discovered in a source file.
///
/// Language providers send flat records with zero-based physical source lines.
/// The host uses their ranges for outline navigation and cursor-scope display.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Symbol {
    /// The name shown in pickers and breadcrumbs.
    pub name: String,
    /// A short kind label such as `function`, `struct`, or `method`.
    pub kind: String,
    /// Zero-based physical source line where the symbol starts.
    pub line: usize,
    /// Zero-based physical source line where the symbol ends, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    /// Name of the containing symbol, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Finds the narrowest symbol range containing `line`.
pub fn enclosing_symbol(symbols: &[Symbol], line: usize) -> Option<&Symbol> {
    symbols
        .iter()
        .filter(|symbol| symbol.line <= line && symbol.end_line.unwrap_or(symbol.line) >= line)
        .min_by_key(|symbol| {
            symbol
                .end_line
                .unwrap_or(symbol.line)
                .saturating_sub(symbol.line)
        })
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
    /// Exact filenames or filename globs handled by this provider.
    pub filenames: Vec<String>,
    /// Shebang interpreter names handled by this provider (for example `bash`).
    pub shebangs: Vec<String>,
    /// Capabilities declared by this provider.
    pub capabilities: std::collections::HashSet<Capability>,
    /// Tie-breaker when two providers register the same file match +
    /// capability pair (protocol 3+). Higher wins; equal priority keeps
    /// whichever provider was registered first. Defaults to `0`, matching
    /// what a plugin that never sends this field is treated as — such a
    /// plugin can still be outranked by one that explicitly asks for a
    /// higher priority, or itself outrank one asking for a lower priority.
    pub priority: i64,
}

/// The color roles a plugin needs to render matching output, sent as
/// `#rrggbb` hex strings so any theme (built-in or user-defined) works
/// without the plugin having to special-case theme names. Sent alongside
/// `theme` on `init` and `on_theme_change` rather than requiring the plugin
/// to maintain its own dictionary of presets per theme name.
#[derive(Serialize, Clone)]
pub(crate) struct ThemeColorsMsg {
    pub(crate) heading1: String,
    pub(crate) heading2: String,
    pub(crate) heading3: String,
    pub(crate) accent: String,
    pub(crate) dim: String,
    pub(crate) code: String,
    pub(crate) text: String,
}

impl From<&Theme> for ThemeColorsMsg {
    fn from(theme: &Theme) -> Self {
        ThemeColorsMsg {
            heading1: color_to_hex(theme.heading1),
            heading2: color_to_hex(theme.heading2),
            heading3: color_to_hex(theme.heading3),
            accent: color_to_hex(theme.accent),
            dim: color_to_hex(theme.dim),
            code: color_to_hex(theme.code),
            text: color_to_hex(theme.text),
        }
    }
}

/// Message sent from `mantis` to a plugin (on its stdin).
#[derive(Serialize)]
pub(crate) struct ToPlugin {
    pub(crate) event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    /// Zero-based physical source line associated with a selection change.
    /// Omitted for tree-only selections and events without a content cursor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) line: Option<usize>,
    /// One-based source column for `on_content_cursor_change` events.
    /// Omitted for events without a content cursor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) column: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) theme: Option<String>,
    /// The active theme's actual colors, so plugins can render without
    /// hardcoding a palette per theme name. Sent on `init` and
    /// `on_theme_change`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) colors: Option<ThemeColorsMsg>,
    /// Protocol version spoken by the host. Present only on the `init` event
    /// so the plugin can verify compatibility.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) protocol_version: Option<String>,
    /// Request id (protocol 3+). Present only on `request` events; echoed
    /// back unchanged by the plugin's `response`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) id: Option<u64>,
    /// Capability-specific method name (protocol 3+). Present only on
    /// `request` events, e.g. `"fold_regions"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) method: Option<String>,
    /// Method-specific parameters (protocol 3+). Present only on `request`
    /// events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) params: Option<serde_json::Value>,
}

/// The `error` object of a plugin's `response` message (protocol 3+).
#[derive(Deserialize, Clone, Debug)]
pub(crate) struct PluginResponseError {
    pub(crate) message: String,
}

/// Message received from a plugin (on its stdout). Covers both `action`
/// messages (the protocol 2 shape) and `response` messages (protocol 3+,
/// correlated to a host `request` by `id`). The reader thread in
/// `crate::plugin::process` dispatches on `event` and routes `action` and
/// `response` messages onto separate channels so a response is never
/// misinterpreted as an action.
#[derive(Deserialize)]
pub(crate) struct FromPlugin {
    pub(crate) event: String,
    pub(crate) action: Option<String>,
    #[serde(default)]
    pub(crate) params: serde_json::Value,
    /// Present on `response` messages: echoes the `id` from the host's `request`.
    #[serde(default)]
    pub(crate) id: Option<u64>,
    /// Present on a successful `response`.
    #[serde(default)]
    pub(crate) result: Option<serde_json::Value>,
    /// Present on a failed `response`.
    #[serde(default)]
    pub(crate) error: Option<PluginResponseError>,
}

/// A single command a plugin contributes to the Ctrl-P command palette.
///
/// Received via the `register_commands` action (`{commands: [{id, name,
/// category?, description?}]}`). When the user selects one, the host sends
/// a `command` event back to the plugin with the command's `id`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PluginCommand {
    /// Stable identifier dispatched back to the plugin on selection.
    pub id: String,
    /// Display name shown in the palette.
    pub name: String,
    /// Optional category label (e.g. "Plugin") for grouping in the palette.
    #[serde(default)]
    pub category: Option<String>,
    /// Optional one-line description shown dim in the palette.
    #[serde(default)]
    pub description: Option<String>,
}

/// Target kinds for plugin-contributed context-menu items.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ContextItemTargetKind {
    /// A file row in the file tree.
    TreeFile,
    /// A directory row in the file tree.
    TreeDir,
    /// The active content editor / viewer pane.
    Content,
    /// A workspace tab in the tab bar.
    Tab,
    /// A git blame annotation row.
    Blame,
}

impl ContextItemTargetKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TreeFile => "tree_file",
            Self::TreeDir => "tree_dir",
            Self::Content => "content",
            Self::Tab => "tab",
            Self::Blame => "blame",
        }
    }
}

/// A context-menu item contributed by a plugin.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PluginContextItem {
    /// Stable identifier dispatched back to the plugin on selection.
    pub id: String,
    /// Display label shown in the context menu.
    pub label: String,
    /// Target kind where this menu item should appear.
    pub target: ContextItemTargetKind,
    /// Optional file extensions (e.g. `["rs", "toml"]`) this item matches.
    #[serde(default)]
    pub extensions: Option<Vec<String>>,
    /// Optional submenu category label for grouping.
    #[serde(default)]
    pub category: Option<String>,
    /// Optional ordering weight (lower appears earlier).
    #[serde(default)]
    pub weight: Option<i32>,
}

/// Which side of the status bar a plugin segment prefers.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum StatusSidePreference {
    #[default]
    Left,
    Right,
}

/// A status-bar segment contributed by a plugin.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PluginStatusSegment {
    /// Stable identifier used for allowlist and `[statusbar.colors]`.
    pub id: String,
    /// Display text shown in the status bar.
    pub text: String,
    /// Elision ladder priority: 1 (dropped first) to 5 (always kept).
    /// Defaults to 2 (`P_INFO`).
    #[serde(default)]
    pub priority: Option<u8>,
    /// Side preference (`left` or `right`). Defaults to `left`.
    #[serde(default)]
    pub side: Option<StatusSidePreference>,
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
    /// Paths in `plugin_status_facts` registered by this plugin.
    pub(crate) status_fact_paths: HashSet<PathBuf>,
    /// Paths in `plugin_symbols` registered by this plugin.
    pub(crate) symbol_paths: HashSet<PathBuf>,
    /// Whether this plugin set the icon map / icon fields via `set_icon_map`.
    pub(crate) has_icon_map: bool,
    /// Command IDs registered by this plugin via `register_commands`.
    pub(crate) command_ids: HashSet<String>,
    /// Context item IDs registered by this plugin via `register_context_items`.
    pub(crate) context_item_ids: HashSet<String>,
    /// Status segment IDs registered by this plugin via `register_status_segments`.
    pub(crate) status_segment_ids: HashSet<String>,
}
