//! Per-root workspace session persistence for `tv`.
//!
//! Persists and restores expanded directories, the last-opened file,
//! scroll/active-line position, and git-mode state across `tv` restarts.
//! State is cached outside the working directory (under
//! `$XDG_STATE_HOME/tree-viewer/` on Linux/macOS, `%APPDATA%\\tree-viewer\\`
//! on Windows) so it survives re-clones and never litters the project tree.
//!
//! The on-disk format is a single `sessions.json` mapping canonical root paths
//! to per-root [`SessionState`] structs. Stale or corrupt entries are silently
//! ignored on load. The entry is a JSON object keyed by root: each root maps
//! to a state with `{ expanded, current_file, content_scroll, active_line }`.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Per-root workspace state restored when re-opening the same directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct SessionState {
    /// Absolute paths of expanded directories.
    #[serde(default)]
    pub expanded: Vec<PathBuf>,
    /// Currently open file path, if any.
    pub current_file: Option<PathBuf>,
    /// Vertical scroll offset in the content pane.
    pub content_scroll: usize,
    /// Active line (cursor) in the content pane.
    pub active_line: usize,
}

/// On-disk collection of all persisted sessions.
#[derive(Debug, Serialize, Deserialize)]
struct SessionFile {
    version: u32,
    #[serde(default)]
    sessions: std::collections::HashMap<String, SessionState>,
}

const SESSION_FILE_VERSION: u32 = 1;
const SESSION_FILE_NAME: &str = "sessions.json";

/// Returns the path to the state directory, creating it if absent.
/// Silently returns `None` when the platform has no suitable state dir.
pub fn state_dir() -> Option<PathBuf> {
    let dir = state_dir_raw()?;
    fs::create_dir_all(&dir).ok();
    Some(dir)
}

/// Returns the path to the sessions JSON file.
pub fn sessions_path() -> Option<PathBuf> {
    state_dir().map(|d| d.join(SESSION_FILE_NAME))
}

/// Loads session state for `root` from the cache, or returns `None`.
///
/// Stale (path no longer exists) and corrupt entries are silently skipped;
/// individual fields that reference nonexistent paths inside `root` are
/// filtered out so restoration never produces dangling references.
pub fn load(root: &Path) -> Option<SessionState> {
    let path = sessions_path()?;
    let raw = fs::read_to_string(&path).ok()?;
    let file: SessionFile = serde_json::from_str(&raw).ok()?;
    let key = root_key(root);
    let mut state: SessionState = file.sessions.get(&key)?.clone();

    // Filter out expanded directories that no longer exist.
    state.expanded.retain(|p| p.starts_with(root) && p.is_dir());

    // Filter out current_file that no longer exists.
    if let Some(ref cf) = state.current_file {
        if !cf.starts_with(root) || !cf.exists() {
            state.current_file = None;
        }
    }

    Some(state)
}

/// Saves session state for `root` to the cache.
///
/// This is a full-file write: the entire `sessions.json` is read, the entry
/// for `root` is upserted, and the file is written back. I/O errors are
/// silently ignored so a broken state dir never crashes the viewer.
pub fn save(root: &Path, state: &SessionState) {
    let Some(path) = sessions_path() else {
        return;
    };
    let raw = fs::read_to_string(&path).ok();
    let mut file: SessionFile = raw
        .as_deref()
        .and_then(|r| serde_json::from_str(r).ok())
        .unwrap_or(SessionFile {
            version: SESSION_FILE_VERSION,
            sessions: std::collections::HashMap::new(),
        });
    file.sessions.insert(root_key(root), state.clone());
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        // Write to a sibling temp file then rename so a crash mid-write never
        // leaves a truncated sessions.json.
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// The key used in the sessions map: the canonical (absolute) root path as a
/// string, with trailing slash stripped.
fn root_key(root: &Path) -> String {
    let canonical = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()
            .ok()
            .map(|cwd| cwd.join(root))
            .unwrap_or_else(|| root.to_path_buf())
    };
    // Normalise: strip trailing separator so `/repo/` and `/repo` match.
    // Use lossless encoding: valid UTF-8 paths are kept as-is; non-UTF-8 paths
    // are hex-encoded so distinct byte sequences never map to the same key.
    let bytes = canonical.as_os_str().as_encoded_bytes();
    let s = if let Ok(utf8) = std::str::from_utf8(bytes) {
        utf8.to_string()
    } else {
        format!(
            "hex:{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        )
    };
    s.trim_end_matches(std::path::MAIN_SEPARATOR).to_string()
}

/// Platform-specific state directory.
///
/// Override with the `TV_STATE_DIR` environment variable (used in tests to
/// isolate concurrent writers).
fn state_dir_raw() -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("TV_STATE_DIR") {
        return Some(PathBuf::from(val));
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("tree-viewer"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .map(|base| base.join("tree-viewer"))
    }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod tests;
