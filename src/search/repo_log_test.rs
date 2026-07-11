use super::*;

#[test]
fn repo_log_state_initial_load() {
    let dir = std::env::temp_dir();
    let state = RepoLogState::new(dir);
    assert!(state.query.is_empty());
    assert_eq!(state.selected, 0);
}
