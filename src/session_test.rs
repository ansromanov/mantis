use super::*;
use crate::session::STATE_DIR_ENV_LOCK as SESSION_LOCK;

/// A per-test environment: creates a unique root and state directory under the
/// system temp directory, then points `MANTIS_STATE_DIR` at the isolated state dir
/// so tests never touch the real `~/.local/state/mantis/sessions.json`.
/// Cleans up on drop.
struct TestEnv {
    root: PathBuf,
    state: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("mantis_session_{name}_{}", std::process::id()));
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        std::env::set_var("MANTIS_STATE_DIR", &state);
        TestEnv { root, state }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::remove_var("MANTIS_STATE_DIR");
        fs::remove_dir_all(&self.root).ok();
    }
}

#[test]
fn round_trip_preserves_all_fields() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("round_trip");
    let sub = env.root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(env.root.join("f.txt"), "content").unwrap();

    let state = SessionState {
        expanded: vec![sub],
        current_file: Some(env.root.join("f.txt")),
        content_scroll: 10,
        active_line: 12,
        initial_root: None,
    };

    save(&env.root, &state);
    let loaded = load(&env.root).expect("should load saved session");
    assert_eq!(loaded, state);
}

#[test]
fn save_and_load_empty_state() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("empty");
    let state = SessionState::default();
    save(&env.root, &state);
    let loaded = load(&env.root).unwrap();
    assert!(loaded.expanded.is_empty());
    assert!(loaded.current_file.is_none());
}

#[test]
fn load_returns_none_for_missing_key() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("missing");
    assert!(load(&env.root).is_none());
}

#[test]
fn stale_expanded_dirs_are_filtered() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("stale_expanded");
    let gone = env.root.join("gone");

    let state = SessionState {
        expanded: vec![gone],
        ..SessionState::default()
    };
    save(&env.root, &state);

    let loaded = load(&env.root).unwrap();
    assert!(loaded.expanded.is_empty());
}

#[test]
fn stale_current_file_is_filtered() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("stale_file");
    let gone = env.root.join("gone.txt");

    let state = SessionState {
        current_file: Some(gone),
        ..SessionState::default()
    };
    save(&env.root, &state);

    let loaded = load(&env.root).unwrap();
    assert!(loaded.current_file.is_none());
}

#[test]
fn corrupt_legacy_file_returns_none() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("corrupt_legacy");
    // Write garbage to the isolated sessions.json (legacy format path).
    let legacy = env.state.join("sessions.json");
    fs::write(&legacy, "not json at all").unwrap();

    // load should return None because:
    //   1. migrate_legacy tries to read sessions.json → parse fails → renames to .migrated
    //   2. session_path(root) looks for sessions/<hash>.json → doesn't exist
    assert!(load(&env.root).is_none());

    // Should not crash on save either (save writes the per-root file)
    save(&env.root, &SessionState::default());
    // Now it should load
    assert!(load(&env.root).is_some());
}

#[test]
fn corrupt_per_root_file_returns_none() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("corrupt_per_root");
    // First save a valid state so the per-root file exists
    save(&env.root, &SessionState::default());
    assert!(load(&env.root).is_some());

    // Now overwrite the per-root file with garbage
    let p = session_path(&env.root).unwrap();
    fs::write(&p, "not json at all").unwrap();

    assert!(load(&env.root).is_none());

    // Should not crash on save
    save(&env.root, &SessionState::default());
    assert!(load(&env.root).is_some());
}

#[test]
fn multiple_roots_are_independent() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("multi");

    let d1 = env.root.join("repo1");
    let d2 = env.root.join("repo2");
    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();

    let s1 = SessionState {
        content_scroll: 5,
        ..SessionState::default()
    };
    let s2 = SessionState {
        content_scroll: 10,
        ..SessionState::default()
    };
    save(&d1, &s1);
    save(&d2, &s2);

    assert_eq!(load(&d1).unwrap().content_scroll, 5);
    assert_eq!(load(&d2).unwrap().content_scroll, 10);
}

#[test]
fn root_key_normalises_trailing_separator() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("trail");

    let state = SessionState {
        content_scroll: 7,
        ..SessionState::default()
    };
    save(&env.root, &state);

    // Load with a trailing-slash variant of the same path
    let with_slash: PathBuf = format!("{}/", env.root.display()).into();
    let loaded = load(&with_slash);
    assert_eq!(loaded.unwrap().content_scroll, 7);
}

#[test]
fn concurrent_saves_dont_clobber() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("concurrent");

    let d1 = env.root.join("project_a");
    let d2 = env.root.join("project_b");
    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();

    let s1 = SessionState {
        content_scroll: 42,
        active_line: 7,
        ..SessionState::default()
    };
    let s2 = SessionState {
        content_scroll: 99,
        active_line: 3,
        ..SessionState::default()
    };

    // Simulate two concurrent saves: each writes only its own per-root file,
    // so there is no read-modify-write race on a shared file.
    save(&d1, &s1);
    save(&d2, &s2);

    assert_eq!(load(&d1).unwrap().content_scroll, 42);
    assert_eq!(load(&d2).unwrap().content_scroll, 99);
}

#[test]
fn legacy_migration_preserves_all_roots() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("legacy_migrate");

    let d1 = env.root.join("alpha");
    let d2 = env.root.join("beta");
    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();

    // Manually write a sessions.json in the old format (version + HashMap).
    // JSON-encode the keys (not just interpolate them) since a raw Windows
    // path contains backslashes that aren't valid JSON escapes on their own.
    let key1 = serde_json::to_string(&d1.to_string_lossy()).unwrap();
    let key2 = serde_json::to_string(&d2.to_string_lossy()).unwrap();
    let old_json = format!(
        r#"{{"version":1,"sessions":{{{}:{{"expanded":[],"current_file":null,"content_scroll":5,"active_line":1}},{}:{{"expanded":[],"current_file":null,"content_scroll":10,"active_line":2}}}}}}"#,
        key1, key2
    );
    let legacy = env.state.join("sessions.json");
    fs::write(&legacy, &old_json).unwrap();

    // load triggers migrate_legacy, which should migrate both roots
    let loaded1 = load(&d1).unwrap();
    assert_eq!(loaded1.content_scroll, 5);
    assert_eq!(loaded1.active_line, 1);

    let loaded2 = load(&d2).unwrap();
    assert_eq!(loaded2.content_scroll, 10);
    assert_eq!(loaded2.active_line, 2);

    // Legacy file should have been renamed
    assert!(!legacy.exists());
    assert!(legacy.with_extension("json.migrated").exists());
}

#[test]
fn save_load_round_trip_uses_per_root_file() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let env = TestEnv::new("per_root_path");

    let state = SessionState {
        expanded: vec![],
        current_file: None,
        content_scroll: 3,
        active_line: 0,
        initial_root: None,
    };
    save(&env.root, &state);

    // The per-root file must exist under sessions/<hash>.json
    let p = session_path(&env.root).unwrap();
    assert!(p.exists(), "per-root session file must exist");

    // The legacy sessions.json must NOT exist (it's migrated or not created)
    let legacy = env.state.join("sessions.json");
    assert!(!legacy.exists(), "legacy sessions.json must not be written");

    // The sessions directory must contain exactly one file
    let sessions_dir = env.state.join("sessions");
    assert!(sessions_dir.is_dir());
    let entries: Vec<_> = fs::read_dir(&sessions_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    assert_eq!(entries.len(), 1, "only one per-root file expected");
}

// -- welcome_shown flag -----------------------------------------------------

#[test]
fn welcome_not_shown_by_default() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = TestEnv::new("welcome_default");
    assert!(!is_welcome_shown(), "welcome must not be shown initially");
}

#[test]
fn mark_welcome_creates_flag() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = TestEnv::new("welcome_mark");
    assert!(!is_welcome_shown(), "precondition: not shown");
    mark_welcome_shown();
    assert!(is_welcome_shown(), "welcome must be shown after mark");
    let path = welcome_shown_path().unwrap();
    assert!(path.exists(), "flag file must exist on disk");
}

#[test]
fn welcome_shown_survives_reinit() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = TestEnv::new("welcome_survive");
    mark_welcome_shown();
    assert!(is_welcome_shown(), "precondition: shown after mark");
    // Re-read by simulating a fresh session (state dir is persistent).
    assert!(is_welcome_shown(), "welcome must stay shown on re-check");
}

#[test]
fn welcome_shown_path_returns_some_when_state_dir_available() {
    let _lock = SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _env = TestEnv::new("welcome_path");
    let path = welcome_shown_path();
    assert!(path.is_some(), "welcome_shown_path must return Some");
    assert!(
        path.unwrap().ends_with("welcome_shown.flag"),
        "must end with welcome_shown.flag"
    );
}
