use super::*;
use crate::session::STATE_DIR_ENV_LOCK as USAGE_LOCK;

struct TestEnv {
    _state: std::path::PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let state =
            std::env::temp_dir().join(format!("mantis_usage_{name}_{}", std::process::id()));
        fs::create_dir_all(&state).unwrap();
        std::env::set_var("MANTIS_STATE_DIR", &state);
        TestEnv { _state: state }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::remove_var("MANTIS_STATE_DIR");
        fs::remove_dir_all(&self._state).ok();
    }
}

#[test]
fn record_sets_last_used_and_first_score_is_one() {
    let mut s = UsageStats::default();
    assert!(s.last_used().is_none());
    assert!(s.top_used(5).is_empty());

    s.record("toggle_help");
    assert_eq!(s.last_used(), Some("toggle_help"));
    let top = s.top_used(5);
    assert_eq!(top.len(), 1);
    assert_eq!(top[0], "toggle_help");
    // First use should have score 1.0
    let entry = s.scores.get("toggle_help").unwrap();
    assert!((entry.score - 1.0).abs() < 0.01);
}

#[test]
fn record_increments_score_on_immediate_reuse() {
    let mut s = UsageStats::default();
    s.record("help");
    let score_after_first = s.scores["help"].score;
    // Immediate reuse (days ≈ 0): score = score * 1 + 1 = score + 1
    s.record("help");
    let score_after_second = s.scores["help"].score;
    assert!(
        (score_after_second - (score_after_first + 1.0)).abs() < 0.01,
        "immediate reuse should add ~1.0, got {score_after_second}"
    );
}

#[test]
fn top_used_returns_up_to_n() {
    let mut s = UsageStats::default();
    s.record("a");
    s.record("a");
    s.record("b");
    s.record("c");
    s.record("c");
    s.record("c");

    assert_eq!(s.top_used(2), vec!["c", "a"]);
    assert_eq!(s.top_used(10).len(), 3);
    assert_eq!(s.top_used(0), Vec::<&str>::new());
}

#[test]
fn top_used_tie_break_alphabetical() {
    let mut s = UsageStats::default();
    s.record("zebra");
    s.record("apple");
    // Both have score ≈1.0; alphabetical tie-break: apple < zebra
    assert_eq!(s.top_used(5), vec!["apple", "zebra"]);
}

#[test]
fn top_used_excludes_zero_score() {
    let mut s = UsageStats::default();
    // Manually insert an entry with score 0 (shouldn't happen in practice
    // but tests the filter).
    s.scores.insert(
        "zero".to_string(),
        FrecencyEntry {
            score: 0.0,
            last_used_ts: unix_ts(),
        },
    );
    s.record("active");
    assert_eq!(s.top_used(5), vec!["active"]);
}

#[test]
fn frecency_decay_over_time() {
    let mut s = UsageStats::default();
    // Record "old_cmd" many times, then manually backdate its timestamp
    // so the next record() call applies heavy decay.
    for _ in 0..10 {
        s.record("old_cmd");
    }
    let past_ts = unix_ts().saturating_sub(60 * 86_400); // 60 days ago
    s.scores.get_mut("old_cmd").unwrap().last_used_ts = past_ts;
    // Re-record "old_cmd" — decay is applied: 10 * 0.9^60 ≈ 0.018 + 1 ≈ 1.018
    s.record("old_cmd");

    // Record "new_cmd" fresh — use it several times so it outranks the decayed old.
    s.record("new_cmd");
    s.record("new_cmd");
    s.record("new_cmd");

    let top = s.top_used(5);
    // new_cmd (score ≈4.0) should outrank old_cmd (≈1.018)
    assert_eq!(top[0], "new_cmd");
}

#[test]
fn round_trip_preserves_scores_and_last_used() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("round_trip");

    let mut s = UsageStats::default();
    s.record("toggle_help");
    s.record("toggle_help");
    s.record("reload");
    s.save();

    let loaded = UsageStats::load();
    assert_eq!(loaded.last_used(), Some("reload"));
    // "toggle_help" used twice → score ≈2.0; "reload" used once → score ≈1.0
    assert_eq!(loaded.top_used(5), vec!["toggle_help", "reload"]);
}

#[test]
fn missing_file_returns_default() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("missing");
    let s = UsageStats::load();
    assert!(s.last_used().is_none());
    assert!(s.top_used(5).is_empty());
}

#[test]
fn corrupt_file_returns_default() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("corrupt");
    let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
        panic!("no state dir");
    };
    fs::write(&path, "not json").unwrap();
    let s = UsageStats::load();
    assert!(s.last_used().is_none());
}

#[test]
fn v1_to_v2_migration() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("migrate_v1");

    // Write a v1-format file manually.
    let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
        panic!("no state dir");
    };
    let v1_json = serde_json::json!({
        "version": 1,
        "counts": { "help": 10, "reload": 3 },
        "last_used": "help"
    });
    fs::write(&path, serde_json::to_string(&v1_json).unwrap()).unwrap();

    let loaded = UsageStats::load();
    // Should have migrated to v2.
    assert_eq!(loaded.version, USAGE_FILE_VERSION);
    assert!(loaded.counts.is_empty());
    assert_eq!(loaded.last_used(), Some("help"));
    // Counts became initial scores.
    let help_score = loaded.scores.get("help").unwrap().score;
    let reload_score = loaded.scores.get("reload").unwrap().score;
    assert!((help_score - 10.0).abs() < 0.01);
    assert!((reload_score - 3.0).abs() < 0.01);
    // Top used: help (10) > reload (3)
    assert_eq!(loaded.top_used(5), vec!["help", "reload"]);
}

#[test]
fn v1_migration_persists_v2_file() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("migrate_v1_persist");

    let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
        panic!("no state dir");
    };
    let v1_json = serde_json::json!({
        "version": 1,
        "counts": { "quit": 5 },
        "last_used": "quit"
    });
    fs::write(&path, serde_json::to_string(&v1_json).unwrap()).unwrap();

    // Load migrates and saves.
    let _loaded = UsageStats::load();

    // Re-read from disk — should be v2 now.
    let raw = fs::read_to_string(&path).unwrap();
    let on_disk: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(on_disk["version"], 2);
    assert!(on_disk.get("counts").is_none());
    assert!(on_disk.get("scores").is_some());
}
