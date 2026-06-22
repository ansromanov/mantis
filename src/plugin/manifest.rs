//! Plugin manifest (`plugin.toml`) loading and discovery.
//!
//! A plugin directory is a subdirectory of the global plugin directory that
//! contains a `plugin.toml` manifest file describing the plugin — its name,
//! version, entry point, and optional metadata. This module parses those
//! manifests and discovers all plugins available in the plugin directory at
//! startup, so the UI can show them in the plugin picker without requiring an
//! explicit `[plugins]` entry in `tv.toml`.
//!
//! # Schema
//!
//! ```toml
//! name = "git-tools"
//! version = "0.1.0"
//! description = "git diff on open, git log on H"
//! author = "ansromanov"
//! entry = "run.sh"
//! tv_protocol = "1"
//! platforms = ["linux", "macos"]
//! events = ["on_file_open", "on_keypress"]
//! permissions = ["run_git", "read_files"]
//! ```
//!
//! The `entry` path is resolved relative to the plugin's subdirectory. The
//! `platforms` field, when present, restricts the plugin to specific operating
//! systems using Rust's `std::env::consts::OS` naming. `events` and
//! `permissions` are advisory-only in this phase and not enforced.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::plugin::{PluginEntry, PluginKind};

/// A plugin manifest as declared in `plugin.toml`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PluginManifest {
    /// Human-readable name. Should match the subdirectory name for clarity,
    /// though this is not enforced.
    pub name: String,
    /// Plugin version (semver recommended but not enforced).
    pub version: String,
    /// One-line description shown in the plugin picker.
    #[serde(default)]
    pub description: Option<String>,
    /// Author name or handle.
    #[serde(default)]
    pub author: Option<String>,
    /// Executable path relative to the plugin directory.
    pub entry: String,
    /// Protocol version string (e.g. `"1"`). Indicates which version of the
    /// tv plugin IPC protocol this plugin expects.
    pub tv_protocol: String,
    /// Optional list of platforms this plugin supports. If absent, all
    /// platforms are assumed. Values use Rust's `std::env::consts::OS`
    /// conventions: `"linux"`, `"macos"`, `"windows"`.
    #[serde(default)]
    pub platforms: Option<Vec<String>>,
    /// Events this plugin handles (advisory only, not enforced).
    #[serde(default)]
    pub events: Option<Vec<String>>,
    /// Permissions this plugin requires (advisory only, not enforced).
    #[serde(default)]
    pub permissions: Option<Vec<String>>,
}

/// Loads a `PluginManifest` from the `plugin.toml` file inside `dir`.
///
/// Returns `None` if the file is missing or cannot be parsed.
pub fn load(dir: &Path) -> Option<PluginManifest> {
    let path = dir.join("plugin.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&content).ok()
}

/// Discovers all plugin manifests in the given plugin directory.
///
/// Scans each immediate subdirectory of `plugin_dir` for a `plugin.toml` file,
/// loads the manifest, and returns a `Vec` of `(name, PluginEntry)` pairs.
/// Entries are filtered by platform: a manifest that lists `platforms` must
/// include the current OS, otherwise the entry is skipped.
///
/// All discovered entries are returned with `enabled = false` so no freshly
/// fetched code runs without explicit user opt-in. The entry `path` is set to
/// `<dir_name>/<entry>` (relative to `plugin_dir`) so that `PluginManager`
/// resolves it correctly against `default_plugin_dir()`. The actual subdirectory
/// name is always used for the path, regardless of the `name` field in the
/// manifest, to prevent path traversal via crafted manifests.
pub fn discover(plugin_dir: &Path) -> Vec<(String, PluginEntry)> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(plugin_dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(manifest) = load(&path) else {
            continue;
        };
        if !is_safe_name(&manifest.name) || !is_safe_entry(&manifest.entry) {
            continue;
        }
        if let Some(ref platforms) = manifest.platforms {
            if !platform_matches(platforms) {
                continue;
            }
        }
        let entry_path: PathBuf = [dir_name, &manifest.entry].iter().collect();
        entries.push((
            manifest.name.clone(),
            PluginEntry {
                path: entry_path,
                enabled: false,
                kind: PluginKind::Process,
                extensions: Vec::new(),
                syntax_file: None,
            },
        ));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

/// Returns `false` if `s` (a manifest `name` value) contains path separators
/// or `..`, which would allow it to escape the plugin directory when used as a
/// single `PathBuf` component.
fn is_safe_name(s: &str) -> bool {
    !s.contains("..") && !s.contains('/') && !s.contains('\\')
}

/// Returns `false` if `s` (a manifest `entry` value) would escape the plugin
/// subdirectory. Forward slashes are allowed for subdirectory-relative paths
/// (e.g. `"bin/myplugin"`), but `..` components and absolute paths are not.
fn is_safe_entry(s: &str) -> bool {
    !s.starts_with('/') && !s.starts_with('\\') && !Path::new(s).components().any(|c| {
        matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir)
    })
}

/// Checks whether the current platform matches one of the given platform names.
/// Comparison is case-insensitive; `std::env::consts::OS` returns lowercase
/// (`"linux"`, `"macos"`, `"windows"`), but manifest authors may use any case.
fn platform_matches(platforms: &[String]) -> bool {
    let current = std::env::consts::OS;
    platforms.iter().any(|p| p.to_lowercase() == current)
}
