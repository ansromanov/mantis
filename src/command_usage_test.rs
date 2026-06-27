use std::sync::Mutex;

use super::*;

static USAGE_LOCK: Mutex<()> = Mutex::new(());

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
fn record_increments_count_and_sets_last_used() {
    let mut s = UsageStats::default();
    assert!(s.last_used().is_none());
    assert!(s.top_used(5).is_empty());

    s.record("toggle_help");
    assert_eq!(s.last_used(), Some("toggle_help"));
    assert_eq!(s.top_used(5), vec!["toggle_help"]);

    s.record("toggle_help");
    assert_eq!(s.top_used(5), vec!["toggle_help"]);

    s.record("reload");
    assert_eq!(s.last_used(), Some("reload"));
    let top = s.top_used(5);
    assert_eq!(top[0], "toggle_help");
    assert_eq!(top[1], "reload");
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
    assert_eq!(s.top_used(5), vec!["apple", "zebra"]);
}

#[test]
fn round_trip_preserves_counts_and_last_used() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("round_trip");

    let mut s = UsageStats::default();
    s.record("toggle_help");
    s.record("toggle_help");
    s.record("reload");
    s.save();

    let loaded = UsageStats::load();
    assert_eq!(loaded.last_used(), Some("reload"));
    // "toggle_help" has count 2, "reload" has count 1
    assert_eq!(loaded.top_used(5), vec!["toggle_help", "reload"]);
}

#[test]
fn missing_file_returns_default() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("missing");
    // Don't save anything — file won't exist.
    let s = UsageStats::load();
    assert!(s.last_used().is_none());
    assert!(s.top_used(5).is_empty());
}

#[test]
fn corrupt_file_returns_default() {
    let _lock = USAGE_LOCK.lock().unwrap();
    let _env = TestEnv::new("corrupt");
    // Write garbage
    let Some(path) = crate::session::state_dir().map(|d| d.join(USAGE_FILE_NAME)) else {
        panic!("no state dir");
    };
    fs::write(&path, "not json").unwrap();
    let s = UsageStats::load();
    assert!(s.last_used().is_none());
}
