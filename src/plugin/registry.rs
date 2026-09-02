//! Git-backed plugin registry (index.json) + local cache.
//!
//! A remote registry is a git repository containing an `index.json` file that
//! lists available plugins with their repo URL and tag. The registry is cloned
//! into `~/.config/mantis/registry/` (or `$XDG_CONFIG_HOME/mantis/registry/`)
//! and refreshed via `git pull`. No HTTP crate is used — all communication with
//! the registry happens through the `git` CLI. Registry refreshes are pinned to
//! the `main` branch and recover by cloning a clean cache when an existing
//! checkout cannot be updated safely. Plugin artifacts can be checked against
//! the SHA-256 digest recorded in the index before they are installed.
//!
//! The default registry URL is a GitHub repo, overridable via the
//! `MANTIS_PLUGIN_REGISTRY` environment variable.
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
//! - `verify_artifact` — validate a downloaded artifact against its index digest

#![allow(dead_code)]
// All items are `pub` API surface ready for the plugin search/install UI
// once wired up in a follow-up PR.

use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Default remote registry repository URL.
///
/// Override by setting the `MANTIS_PLUGIN_REGISTRY` environment variable.
pub const DEFAULT_REGISTRY_REPO: &str = "https://github.com/ansromanov/mantis-plugins";

/// A single plugin entry in the registry index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RegistryEntry {
    pub name: String,
    pub description: String,
    pub repo: String,
    pub tag: String,
    /// SHA-256 digest of the released artifact, encoded as lowercase hex.
    /// Missing digests are accepted for backwards-compatible index parsing but
    /// rejected by [`verify_artifact`].
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Top-level structure of `index.json`.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct RegistryIndex {
    pub plugins: Vec<RegistryEntry>,
}

/// Returns the path to the local registry cache directory.
///
/// Uses the same config-directory resolution as the rest of `mantis`:
/// - `$MANTIS_PLUGIN_REGISTRY_DIR` env var (absolute override)
/// - `$XDG_CONFIG_HOME/mantis/registry/` (Linux/macOS)
/// - `~/.config/mantis/registry/` (fallback)
/// - `%APPDATA%\mantis\registry\` (Windows)
pub fn registry_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MANTIS_PLUGIN_REGISTRY_DIR") {
        return PathBuf::from(dir);
    }
    config_dir().join("registry")
}

fn config_dir() -> PathBuf {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(|p| PathBuf::from(p).join("mantis"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("mantis"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Returns the registry repo URL, respecting the `MANTIS_PLUGIN_REGISTRY` override.
fn registry_repo() -> String {
    std::env::var("MANTIS_PLUGIN_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY_REPO.to_string())
}

/// Ensures the local registry cache exists and is up to date.
///
/// If the cache directory does not exist, performs a single-branch clone of
/// `main`. Existing checkouts are updated with a fast-forward-only pull. If
/// that update fails, a clean clone is prepared beside the cache and swapped
/// in atomically enough to preserve the last known-good checkout when recovery
/// also fails.
pub fn clone_or_pull() -> Result<(), String> {
    let dir = registry_dir();
    let repo = registry_repo();

    if dir.join(".git").is_dir() {
        let branch = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
            .output()
            .map_err(|e| format!("failed to inspect registry branch: {e}"))?;
        if !branch.status.success() || String::from_utf8_lossy(&branch.stdout).trim() != "main" {
            return refresh_clean(&dir, &repo)
                .map_err(|e| format!("registry cache is not on main; recovery failed: {e}"));
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["pull", "--ff-only", "-q", "origin", "main"])
            .output()
            .map_err(|e| format!("failed to run git pull: {e}"))?;

        if output.status.success() && dir.join("index.json").is_file() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let pull_error = if stderr.trim().is_empty() {
            "registry checkout is missing index.json".to_string()
        } else {
            format!("git pull failed: {}", stderr.trim())
        };
        return refresh_clean(&dir, &repo)
            .map_err(|e| format!("{pull_error}; recovery failed: {e}"));
    }

    refresh_clean(&dir, &repo)
}

fn refresh_clean(dir: &std::path::Path, repo: &str) -> Result<(), String> {
    let Some(parent) = dir.parent() else {
        return Err("registry cache has no parent directory".into());
    };
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create registry directory: {e}"))?;
    let temp = parent.join(format!(".registry.tmp.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    let output = Command::new("git")
        .args(["clone", "-q", "--branch", "main", "--single-branch", repo])
        .arg(&temp)
        .output()
        .map_err(|e| format!("failed to run git clone: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = std::fs::remove_dir_all(&temp);
        return Err(format!("git clone failed: {}", stderr.trim()));
    }
    if !temp.join("index.json").is_file() {
        let _ = std::fs::remove_dir_all(&temp);
        return Err("cloned registry does not contain index.json".into());
    }
    replace_cache(dir, &temp)
}

fn replace_cache(dir: &std::path::Path, temp: &std::path::Path) -> Result<(), String> {
    let backup = dir
        .parent()
        .ok_or_else(|| "registry cache has no parent directory".to_string())?
        .join(format!(".registry.backup.{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&backup);
    let had_cache = dir.exists();
    if had_cache {
        std::fs::rename(dir, &backup)
            .map_err(|e| format!("failed to stage old registry cache: {e}"))?;
    }
    if let Err(error) = std::fs::rename(temp, dir) {
        if had_cache {
            let _ = std::fs::rename(&backup, dir);
        }
        let _ = std::fs::remove_dir_all(temp);
        return Err(format!("failed to install fresh registry cache: {error}"));
    }
    if had_cache {
        let _ = std::fs::remove_dir_all(backup);
    }
    Ok(())
}

/// Verifies an artifact against the SHA-256 digest recorded for a registry entry.
///
/// A missing or malformed digest is rejected. The returned error is suitable for
/// surfacing in the plugin installation UI and includes no artifact contents.
pub fn verify_artifact(path: &std::path::Path, entry: &RegistryEntry) -> Result<(), String> {
    let expected = entry
        .sha256
        .as_deref()
        .ok_or_else(|| format!("plugin '{}' has no SHA-256 checksum", entry.name))?;
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(format!(
            "plugin '{}' has an invalid SHA-256 checksum",
            entry.name
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("failed to read plugin artifact '{}': {e}", path.display()))?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual == expected {
        Ok(())
    } else {
        Err(format!("checksum mismatch for plugin '{}'", entry.name))
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
