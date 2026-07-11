//! Global command-palette usage stats with frecency ranking.
//!
//! Tracks how recently and frequently each action_id has been invoked, using a
//! classic frecency decay formula: `score = score * 0.9^days + 1` on each use.
//! Used by [`crate::command_palette`] to rank the palette's empty-query view —
//! the most-recently-used command is pinned at the top, followed by the
//! highest-scoring ones (controlled by `palette_pin_recent` and
//! `palette_frequent_count` in [`crate::config::Config`]).
//!
//! Stats are persisted to `$STATE_DIR/command_usage.json` (same state directory as
//! session history; see `session.rs`). Saves are atomic (temp-file + rename).
//! Missing or corrupt files silently fall back to defaults — no usage data is ever
//! required for the app to function.
//!
//! The file format is versioned: v1 used raw counts, v2 (current) stores
//! `(score, last_used_ts)` per action. On load, v1 files are automatically
//! migrated to v2 (counts become initial scores, timestamps set to now).
//!
//! Public items:
//! - [`UsageStats`] — persisted stats structure exposing `load`, `record`,
//!   `last_used`, `top_used`, and `save`.

use std::collections::HashMap;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const USAGE_FILE_VERSION: u32 = 2;
const USAGE_FILE_NAME: &str = "command_usage.json";
/// Frecency decay base: each day without use multiplies the score by this.
const DECAY_BASE: f64 = 0.9;
const SECS_PER_DAY: f64 = 86_400.0;

/// A single action's frecency state: its accumulated score and the unix
/// timestamp (seconds) of its most recent invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct FrecencyEntry {
    score: f64,
    last_used_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    version: u32,
    /// action_id -> frecency score and last-use timestamp.
    #[serde(default)]
    scores: HashMap<String, FrecencyEntry>,
    /// action_id of the most recently invoked command, if any.
    #[serde(default)]
    last_used: Option<String>,
    /// V1 compat: raw invocation counts, populated only when loading a v1 file
    /// during migration. Never serialized in v2 output.
    #[serde(default, skip_serializing)]
    counts: HashMap<String, u64>,
}

impl Default for UsageStats {
    fn default() -> Self {
        UsageStats {
            version: USAGE_FILE_VERSION,
            scores: HashMap::new(),
            last_used: None,
            counts: HashMap::new(),
        }
    }
}

impl UsageStats {
    /// Load from disk; returns `Default` if the file is missing or corrupt.
    /// V1 files are automatically migrated to v2 on load.
    pub fn load() -> Self {
        let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
            return Self::default();
        };
        let Ok(raw) = fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(mut stats) = serde_json::from_str::<UsageStats>(&raw) else {
            return Self::default();
        };
        if stats.version < USAGE_FILE_VERSION {
            stats.migrate_v1_to_v2();
        }
        stats
    }

    /// Record one invocation of `action_id` (apply frecency decay, mark as
    /// last-used). Caller is responsible for calling `save()` afterwards.
    pub fn record(&mut self, action_id: &str) {
        let now = unix_ts();
        let entry = self
            .scores
            .entry(action_id.to_string())
            .or_insert(FrecencyEntry {
                score: 0.0,
                last_used_ts: now,
            });
        let days = now.saturating_sub(entry.last_used_ts) as f64 / SECS_PER_DAY;
        entry.score = entry.score * DECAY_BASE.powf(days) + 1.0;
        entry.last_used_ts = now;
        self.last_used = Some(action_id.to_string());
    }

    /// The most-recently-used action_id, if any.
    pub fn last_used(&self) -> Option<&str> {
        self.last_used.as_deref()
    }

    /// The `n` highest-scoring action_ids, with frecency decay applied at
    /// read time so entries last recorded at different moments compare on
    /// their current (decayed) scores. Ties broken by action_id
    /// (alphabetical) so ordering is deterministic. Actions with a zero/absent
    /// score are not returned.
    pub fn top_used(&self, n: usize) -> Vec<&str> {
        let now = unix_ts();
        let mut v: Vec<(&str, f64)> = self
            .scores
            .iter()
            .filter(|(_, e)| e.score > 0.0)
            .map(|(k, e)| {
                let days = now.saturating_sub(e.last_used_ts) as f64 / SECS_PER_DAY;
                (k.as_str(), e.score * DECAY_BASE.powf(days))
            })
            .collect();
        v.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(b.0))
        });
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

    /// Migrate a v1 file (raw counts) to v2 (frecency scores). Each count
    /// becomes its initial score with `last_used_ts` set to now so decay
    /// begins from the migration point.
    fn migrate_v1_to_v2(&mut self) {
        let now = unix_ts();
        for (action_id, count) in std::mem::take(&mut self.counts) {
            self.scores.insert(
                action_id,
                FrecencyEntry {
                    score: count as f64,
                    last_used_ts: now,
                },
            );
        }
        self.version = USAGE_FILE_VERSION;
        self.save();
    }
}

/// Current unix timestamp in seconds.
fn unix_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[path = "command_usage_test.rs"]
mod tests;
