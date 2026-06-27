//! Global command-palette usage stats: how many times each action_id has been
//! invoked, and which was invoked most recently. Used by [`crate::command_palette`]
//! to rank the palette's empty-query view — the most-recently-used command is
//! pinned at the top, followed by the most-frequently-used ones (controlled by
//! `palette_pin_recent` and `palette_frequent_count` in [`crate::config::Config`]).
//!
//! Stats are persisted to `$STATE_DIR/command_usage.json` (same state directory as
//! session history; see `session.rs`). Saves are atomic (temp-file + rename).
//! Missing or corrupt files silently fall back to defaults — no usage data is ever
//! required for the app to function.
//!
//! Public items:
//! - [`UsageStats`] — persisted stats structure exposing `load`, `record`,
//!   `last_used`, `top_used`, and `save`.

use std::collections::HashMap;
use std::fs;

use serde::{Deserialize, Serialize};

const USAGE_FILE_VERSION: u32 = 1;
const USAGE_FILE_NAME: &str = "command_usage.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    version: u32,
    /// action_id -> number of times invoked.
    #[serde(default)]
    counts: HashMap<String, u64>,
    /// action_id of the most recently invoked command, if any.
    #[serde(default)]
    last_used: Option<String>,
}

impl Default for UsageStats {
    fn default() -> Self {
        UsageStats {
            version: USAGE_FILE_VERSION,
            counts: HashMap::new(),
            last_used: None,
        }
    }
}

impl UsageStats {
    /// Load from disk; returns `Default` if the file is missing or corrupt.
    pub fn load() -> Self {
        let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
            return Self::default();
        };
        fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Record one invocation of `action_id` (increment count, mark as last-used).
    /// Caller is responsible for calling `save()` afterwards.
    pub fn record(&mut self, action_id: &str) {
        *self.counts.entry(action_id.to_string()).or_insert(0) += 1;
        self.last_used = Some(action_id.to_string());
    }

    /// The most-recently-used action_id, if any.
    pub fn last_used(&self) -> Option<&str> {
        self.last_used.as_deref()
    }

    /// The `n` most-used action_ids, highest count first. Ties broken by
    /// action_id (alphabetical) so ordering is deterministic. Actions with a
    /// zero/absent count are not returned.
    pub fn top_used(&self, n: usize) -> Vec<&str> {
        let mut v: Vec<(&str, u64)> = self.counts.iter().map(|(k, &c)| (k.as_str(), c)).collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        v.into_iter().take(n).map(|(id, _)| id).collect()
    }

    /// Atomic save (temp file + rename), mirroring `session::save`. I/O errors
    /// are silently ignored.
    pub fn save(&self) {
        let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
            return;
        };
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if fs::write(&tmp, &json).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }
}

#[cfg(test)]
#[path = "command_usage_test.rs"]
mod tests;
