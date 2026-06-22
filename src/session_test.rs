use std::sync::Mutex;

use super::*;

/// Serialises all session tests so the shared `sessions.json` file is never
/// written by two tests concurrently when `cargo test` runs in parallel.
static SESSION_LOCK: Mutex<()> = Mutex::new(());

/// A per-test environment: creates a unique root directory under the
/// system temp directory. Cleans up on drop.
struct TestEnv {
    root: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("tv_session_{name}_{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        TestEnv { root }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

#[test]
fn round_trip_preserves_all_fields() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("round_trip");
    let sub = env.root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(env.root.join("f.txt"), "content").unwrap();

    let state = SessionState {
        expanded: vec![sub],
        current_file: Some(env.root.join("f.txt")),
        content_scroll: 10,
        active_line: 12,
        git_mode: true,
    };

    save(&env.root, &state);
    let loaded = load(&env.root).expect("should load saved session");
    assert_eq!(loaded, state);
}

#[test]
fn save_and_load_empty_state() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("empty");
    let state = SessionState::default();
    save(&env.root, &state);
    let loaded = load(&env.root).unwrap();
    assert!(loaded.expanded.is_empty());
    assert!(loaded.current_file.is_none());
    assert!(!loaded.git_mode);
}

#[test]
fn load_returns_none_for_missing_key() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("missing");
    // No save was called, so there's no entry for this root.
    assert!(load(&env.root).is_none());
}

#[test]
fn stale_expanded_dirs_are_filtered() {
    let _lock = SESSION_LOCK.lock().unwrap();
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
    let _lock = SESSION_LOCK.lock().unwrap();
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
fn corrupt_file_returns_none() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("corrupt");
    // Write garbage to the sessions file
    let mut p = state_dir().unwrap();
    p.push(SESSION_FILE_NAME);
    fs::write(&p, "not json at all").unwrap();

    assert!(load(&env.root).is_none());

    // Should not crash on save either
    save(&env.root, &SessionState::default());
}

#[test]
fn multiple_roots_are_independent() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("multi");

    let d1 = env.root.join("repo1");
    let d2 = env.root.join("repo2");
    fs::create_dir_all(&d1).unwrap();
    fs::create_dir_all(&d2).unwrap();

    let s1 = SessionState {
        git_mode: true,
        ..SessionState::default()
    };
    let s2 = SessionState {
        git_mode: false,
        ..SessionState::default()
    };
    save(&d1, &s1);
    save(&d2, &s2);

    assert!(load(&d1).unwrap().git_mode);
    assert!(!load(&d2).unwrap().git_mode);
}

#[test]
fn root_key_normalises_trailing_separator() {
    let _lock = SESSION_LOCK.lock().unwrap();
    let env = TestEnv::new("trail");

    let state = SessionState {
        git_mode: true,
        ..SessionState::default()
    };
    save(&env.root, &state);

    // Load with a trailing-slash variant of the same path
    let with_slash: PathBuf = format!("{}/", env.root.display()).into();
    let loaded = load(&with_slash);
    assert!(loaded.unwrap().git_mode);
}
