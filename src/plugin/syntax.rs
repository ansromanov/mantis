//! Syntax plugin discovery and loading.
//!
//! Collects syntax definitions from explicit `[plugins]` entries whose
//! `kind = "syntax"` and auto-discovers `.sublime-syntax` files in the
//! `{plugin_dir}/syntaxes/` directory. Deduplicates by path and sorts
//! for deterministic loading.

use std::path::PathBuf;

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
///
/// Skips any file whose path matches the `syntax_file` of a registered `[plugins]`
/// entry, so that those entries control whether their syntax is loaded via
/// `enabled` (handled by [`collect_syntax_plugins`]) rather than being loaded
/// unconditionally.
pub(crate) fn discover_syntax_plugins(entries: &[(String, PluginEntry)]) -> Vec<ExtraSyntax> {
    use std::collections::HashSet;

    let syntax_dir = default_plugin_dir().join("syntaxes");
    if !syntax_dir.is_dir() {
        return Vec::new();
    }

    // Collect paths already managed by a [plugins] entry — their enabled/disabled
    // state is handled by collect_syntax_plugins.
    let managed: HashSet<PathBuf> = entries
        .iter()
        .filter_map(|(_, e)| e.syntax_file.as_ref())
        .map(|p| {
            if p.is_relative() {
                default_plugin_dir().join(p)
            } else {
                p.clone()
            }
        })
        .collect();

    let mut extra = Vec::new();
    if let Ok(dir_entries) = std::fs::read_dir(&syntax_dir) {
        for dir_entry in dir_entries.flatten() {
            let path = dir_entry.path();
            if path.extension().is_some_and(|e| e == "sublime-syntax") && !managed.contains(&path) {
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
    extra.extend(discover_syntax_plugins(entries));
    let mut seen = std::collections::HashSet::new();
    extra.retain(|e| seen.insert(e.syntax_path.clone()));
    extra.sort_by(|a, b| a.syntax_path.cmp(&b.syntax_path));
    extra
}
