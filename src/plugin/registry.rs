//! Git-backed plugin registry (index.json) + local cache.
//!
//! A remote registry is a git repository containing an `index.json` file that
//! lists available plugins with their repo URL and tag. The registry is cloned
//! into `~/.config/tree-viewer/registry/` (or `$XDG_CONFIG_HOME/tree-viewer/registry/`)
//! and refreshed via `git pull`. No HTTP crate is used — all communication with
//! the registry happens through the `git` CLI.
//!
//! The default registry URL is a GitHub repo, overridable via the
//! `TV_PLUGIN_REGISTRY` environment variable.
//!
//! # Public items
//!
//! - `DEFAULT_REGISTRY_REPO` — default git remote URL
//! - `RegistryEntry` — a single plugin listing in the index
//! - `RegistryIndex` — the top-level JSON structure (`{ "plugins": [...] }`)
//! - `registry_dir` — local cache path for the cloned registry repo
//! - `clone_or_pull` — fresh clone or `git pull` to update the cache
//! - `load_index` — parse `index.json` from the cache directory
//! - `search` — substring match on name/description
//! - `resolve` — find a single entry by exact name

#![allow(dead_code)]
// All items are `pub` API surface ready for the plugin search/install UI
// once wired up in a follow-up PR.

use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

/// Default remote registry repository URL.
///
/// Override by setting the `TV_PLUGIN_REGISTRY` environment variable.
pub const DEFAULT_REGISTRY_REPO: &str = "https://github.com/ansromanov/tree-viewer-plugins";

/// A single plugin entry in the registry index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegistryEntry {
    pub name: String,
    pub description: String,
    pub repo: String,
    pub tag: String,
}

/// Top-level structure of `index.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RegistryIndex {
    pub plugins: Vec<RegistryEntry>,
}

/// Returns the path to the local registry cache directory.
///
/// Uses the same config-directory resolution as the rest of `tv`:
/// - `$TV_PLUGIN_REGISTRY_DIR` env var (absolute override)
/// - `$XDG_CONFIG_HOME/tree-viewer/registry/` (Linux/macOS)
/// - `~/.config/tree-viewer/registry/` (fallback)
/// - `%APPDATA%\tree-viewer\registry\` (Windows)
pub fn registry_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("TV_PLUGIN_REGISTRY_DIR") {
        return PathBuf::from(dir);
    }
    config_dir().join("registry")
}

fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("tree-viewer"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("tree-viewer"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Returns the registry repo URL, respecting the `TV_PLUGIN_REGISTRY` override.
fn registry_repo() -> String {
    std::env::var("TV_PLUGIN_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY_REPO.to_string())
}

/// Ensures the local registry cache exists and is up to date.
///
/// If the cache directory does not exist, performs a `git clone`. If it does
/// exist, runs `git pull` to refresh. Returns `Ok(())` on success; returns
/// `Err` with a description on any failure (git unavailable, clone/pull error,
/// or permission issues).
pub fn clone_or_pull() -> Result<(), String> {
    let dir = registry_dir();
    let repo = registry_repo();

    if dir.join("index.json").exists() {
        // Refresh via pull.
        let output = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["pull", "--ff-only", "-q"])
            .output()
            .map_err(|e| format!("failed to run git pull: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git pull failed: {}", stderr.trim()));
        }
        Ok(())
    } else {
        // Fresh clone.
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create registry directory: {e}"))?;
        }
        let output = Command::new("git")
            .args(["clone", "-q", &repo])
            .arg(&dir)
            .output()
            .map_err(|e| format!("failed to run git clone: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git clone failed: {}", stderr.trim()));
        }
        Ok(())
    }
}

/// Loads and parses `index.json` from the local registry cache.
///
/// Returns `None` if the file does not exist or cannot be parsed.
pub fn load_index() -> Option<RegistryIndex> {
    let path = registry_dir().join("index.json");
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Searches the registry index for plugins whose `name` or `description`
/// contains the query string (case-insensitive substring match).
///
/// Returns entries sorted by name. An empty query returns every entry.
pub fn search(index: &RegistryIndex, query: &str) -> Vec<RegistryEntry> {
    let query_lower = query.to_lowercase();
    let mut results: Vec<RegistryEntry> = index
        .plugins
        .iter()
        .filter(|e| {
            query_lower.is_empty()
                || e.name.to_lowercase().contains(&query_lower)
                || e.description.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect();
    results.sort_by(|a, b| a.name.cmp(&b.name));
    results
}

/// Find a single plugin by exact name match (case-sensitive).
pub fn resolve<'a>(index: &'a RegistryIndex, name: &str) -> Option<&'a RegistryEntry> {
    index.plugins.iter().find(|e| e.name == name)
}

#[cfg(test)]
#[path = "registry_test.rs"]
mod registry_tests;
