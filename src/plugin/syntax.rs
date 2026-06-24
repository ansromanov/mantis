//! Syntax plugin discovery and loading.
//!
//! Collects syntax definitions from explicit `[plugins]` entries whose
//! `kind = "syntax"` and auto-discovers `.sublime-syntax` files in the
//! `{plugin_dir}/syntaxes/` directory. Deduplicates by path and sorts
//! for deterministic loading.

use crate::plugin::install::default_plugin_dir;
use crate::plugin::types::{ExtraSyntax, PluginEntry, PluginKind};

/// Collects `ExtraSyntax` entries from `[plugins]` entries whose
/// `kind = "syntax"`. The `syntax_file` path is resolved against the default
/// plugin directory when relative.
pub(crate) fn collect_syntax_plugins(entries: &[(String, PluginEntry)]) -> Vec<ExtraSyntax> {
    let plugin_dir = default_plugin_dir();
    entries
        .iter()
        .filter(|(_, e)| e.kind == PluginKind::Syntax && e.enabled)
        .filter_map(|(_, entry)| {
            let syntax_path = entry.syntax_file.as_ref()?;
            let path = if syntax_path.is_relative() {
                plugin_dir.join(syntax_path)
            } else {
                syntax_path.clone()
            };
            Some(ExtraSyntax {
                syntax_path: path,
                extensions: entry.extensions.clone(),
            })
        })
        .collect()
}

/// Auto-discovers `.sublime-syntax` files in `{plugin_dir}/syntaxes/`.
pub(crate) fn discover_syntax_plugins() -> Vec<ExtraSyntax> {
    let syntax_dir = default_plugin_dir().join("syntaxes");
    if !syntax_dir.is_dir() {
        return Vec::new();
    }
    let mut extra = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&syntax_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "sublime-syntax") {
                extra.push(ExtraSyntax {
                    syntax_path: path,
                    extensions: Vec::new(),
                });
            }
        }
    }
    extra
}

/// Combines config-based and auto-discovered syntax plugins into a single
/// deduplicated, sorted list of extra syntax definitions for the highlighter.
pub(crate) fn load_extra_syntaxes(entries: &[(String, PluginEntry)]) -> Vec<ExtraSyntax> {
    let mut extra = collect_syntax_plugins(entries);
    extra.extend(discover_syntax_plugins());
    let mut seen = std::collections::HashSet::new();
    extra.retain(|e| seen.insert(e.syntax_path.clone()));
    extra.sort_by(|a, b| a.syntax_path.cmp(&b.syntax_path));
    extra
}
