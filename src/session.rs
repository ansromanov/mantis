//! Per-root workspace session persistence for `mantis`.
//!
//! Persists and restores expanded directories, the last-opened file,
//! scroll/active-line position, and git-mode state across `mantis` restarts.
//! State is cached outside the working directory (under
//! `$XDG_STATE_HOME/mantis/` on Linux/macOS, `%APPDATA%\\mantis\\`
//! on Windows) so it survives re-clones and never litters the project tree.
//!
//! Each root gets its own file under `sessions/<hash>.json` so concurrent
//! mantis instances on different roots never race. A one-time migration
//! reads the legacy `sessions.json` and creates the per-root files.
//!
//! A global `welcome_shown.flag` file in the state directory tracks whether
//! the first-run welcome overlay has been dismissed, so it is shown exactly
//! once across all roots.

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
    /// The initial root directory that mantis was launched from.
    #[serde(default)]
    pub initial_root: Option<PathBuf>,
}

const SESSION_DIR_NAME: &str = "sessions";
const LEGACY_FILE_NAME: &str = "sessions.json";

/// Serialises every test in the crate that sets the process-global
/// `MANTIS_STATE_DIR` env var (used to point [`state_dir`] at an isolated
/// temp directory). `cargo test` runs tests on separate threads within the
/// same process by default, so any test that sets this var without holding
/// this lock can race with another one and read/write the wrong state dir.
#[cfg(test)]
pub(crate) static STATE_DIR_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Returns the path to the state directory, creating it if absent.
/// Silently returns `None` when the platform has no suitable state dir.
pub fn state_dir() -> Option<PathBuf> {
    let dir = state_dir_raw()?;
    fs::create_dir_all(&dir).ok();
    Some(dir)
}

/// Returns the path to the legacy `sessions.json` file (pre-v0.13 format).
/// Used for migration; tests also write to this path to simulate old-format
/// session data.
pub fn sessions_path() -> Option<PathBuf> {
    state_dir().map(|d| d.join(LEGACY_FILE_NAME))
}

/// Returns the per-root session file path for `root`, creating the
/// `sessions/` directory if necessary.
pub(crate) fn session_path(root: &Path) -> Option<PathBuf> {
    let dir = state_dir()?.join(SESSION_DIR_NAME);
    fs::create_dir_all(&dir).ok()?;
    let hash = hash_root_key(&root_key(root));
    Some(dir.join(format!("{hash}.json")))
}

/// Loads session state for `root` from the cache, or returns `None`.
///
/// Runs a one-time migration of legacy `sessions.json` if present.
/// Stale entries are silently skipped; individual fields that reference
/// nonexistent paths inside `root` are filtered out so restoration never
/// produces dangling references.
pub fn load(root: &Path) -> Option<SessionState> {
    migrate_legacy();
    let path = session_path(root)?;
    let raw = fs::read_to_string(&path).ok()?;
    let mut state: SessionState = serde_json::from_str(&raw).ok()?;

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

/// Saves session state for `root` to its own file.
///
/// Each root writes only its own file — no read-modify-write race.
/// I/O errors are silently ignored so a broken state dir never crashes
/// the viewer.
pub fn save(root: &Path, state: &SessionState) {
    migrate_legacy();
    let Some(path) = session_path(root) else {
        return;
    };
    if let Ok(json) = serde_json::to_string_pretty(state) {
        // Write to a sibling temp file then rename so a crash mid-write never
        // leaves a truncated file.
        let tmp = path.with_extension("json.tmp");
        if fs::write(&tmp, &json).is_ok() {
            let _ = fs::rename(&tmp, &path);
        }
    }
}

/// One-time migration from the legacy single-file format.
///
/// Reads `<state_dir>/sessions.json` (if it exists), writes a per-root file
/// under `sessions/` for each entry, then renames the legacy file to
/// `sessions.json.migrated` so subsequent calls are no-ops — even when the
/// legacy file is corrupt or unreadable, the rename prevents repeated I/O.
fn migrate_legacy() {
    let legacy = match sessions_path() {
        Some(p) if p.exists() => p,
        _ => return,
    };
    // Try to read and migrate. Best-effort: if the file is corrupt, we still
    // rename it so we don't retry every save/load.
    if let Ok(raw) = fs::read_to_string(&legacy) {
        // Legacy format: { version: u32, sessions: { "<root>": SessionState, ... } }
        #[derive(Deserialize)]
        struct LegacyFile {
            #[allow(dead_code)]
            version: u32,
            #[serde(default)]
            sessions: std::collections::HashMap<String, SessionState>,
        }
        if let Ok(legacy_file) = serde_json::from_str::<LegacyFile>(&raw) {
            let dir = match state_dir() {
                Some(d) => d.join(SESSION_DIR_NAME),
                None => return,
            };
            let _ = fs::create_dir_all(&dir);
            for (key, state) in &legacy_file.sessions {
                let hash = hash_root_key(key);
                let p = dir.join(format!("{hash}.json"));
                if let Ok(json) = serde_json::to_string_pretty(state) {
                    let tmp = p.with_extension("json.tmp");
                    if fs::write(&tmp, &json).is_ok() {
                        let _ = fs::rename(&tmp, &p);
                    }
                }
            }
        }
    }
    // Rename legacy file so migration runs only once
    let _ = fs::rename(&legacy, legacy.with_extension("json.migrated"));
}

/// FNV-1a 64-bit hash of the root-key string, formatted as 16 hex chars.
///
/// Produces a short, deterministic, filesystem-safe filename for every root.
fn hash_root_key(key: &str) -> String {
    let mut hash: u64 = 14695981039346656037;
    for byte in key.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    format!("{:016x}", hash)
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
    // Windows accepts both `/` and `\` as path separators, so a trailing
    // `/repo/` must normalise the same as `MAIN_SEPARATOR`-only `/repo\`;
    // trim either regardless of platform rather than just `MAIN_SEPARATOR`.
    s.trim_end_matches(['/', '\\']).to_string()
}

/// Returns the path to the global `welcome_shown.flag` file in the state dir.
/// The file's mere existence (not its content) indicates the welcome overlay
/// has been dismissed.
pub fn welcome_shown_path() -> Option<PathBuf> {
    state_dir().map(|d| d.join("welcome_shown.flag"))
}

/// Returns `true` when the first-run welcome overlay has already been
/// dismissed. Returns `false` when the flag file is absent or the platform
/// has no state dir.
pub fn is_welcome_shown() -> bool {
    welcome_shown_path().is_some_and(|p| p.exists())
}

/// Creates the `welcome_shown.flag` file so the welcome overlay is never
/// shown again. Best-effort: I/O errors are silently ignored.
pub fn mark_welcome_shown() {
    if let Some(path) = welcome_shown_path() {
        let _ = fs::write(&path, "");
    }
}

/// Platform-specific state directory.
///
/// Override with the `MANTIS_STATE_DIR` environment variable (used in tests to
/// isolate concurrent writers).
fn state_dir_raw() -> Option<PathBuf> {
    if let Some(val) = std::env::var_os("MANTIS_STATE_DIR") {
        return Some(PathBuf::from(val));
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
            })
            .map(|base| base.join("mantis"))
    }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod tests;
