use super::*;

#[test]
fn repo_log_state_initial_load() {
    let dir = std::env::temp_dir();
    let state = RepoLogState::new(dir);
    assert!(state.query.is_empty());
    assert_eq!(state.selected, 0);
}

/// Creates a throwaway git repo with `commits` empty commits.
fn temp_repo(commits: usize) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mantis_repo_log_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test"]);
    for i in 0..commits {
        git(&[
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            &format!("commit {i}"),
        ]);
    }
    dir
}

#[test]
fn load_more_preserves_selection() {
    let dir = temp_repo(3);
    let mut state = RepoLogState::new(dir.clone());
    // Simulate a partially loaded list: only the 2 newest commits are in,
    // with the cursor on the last loaded entry.
    state.commits.truncate(2);
    state.total_loaded = 2;
    state.has_more = true;
    state.filtered = vec![0, 1];
    state.selected = 1;
    assert!(state.load_more());
    assert_eq!(state.results_len(), 3);
    assert_eq!(state.selected, 1, "paging must not reset the cursor");
    std::fs::remove_dir_all(&dir).ok();
}
