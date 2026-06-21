use std::collections::HashMap;
use std::path::PathBuf;

use super::*;

#[test]
fn git_head_display_branch() {
    assert_eq!(GitHead::Branch("main".to_string()).display(), "main");
    assert_eq!(
        GitHead::Branch("feat/foo".to_string()).display(),
        "feat/foo"
    );
}

#[test]
fn git_head_display_special_states() {
    assert_eq!(GitHead::Detached.display(), "HEAD (detached)");
    assert_eq!(GitHead::Rebase.display(), "REBASE");
    assert_eq!(GitHead::Merge.display(), "MERGE");
}

#[test]
fn git_head_default_is_detached() {
    assert_eq!(GitHead::default(), GitHead::Detached);
}

#[test]
fn git_repo_info_is_dirty_false_when_clean() {
    let info = GitRepoInfo {
        head: GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    };
    assert!(!info.is_dirty());
}

#[test]
fn git_repo_info_is_dirty_true_when_changed() {
    let info = GitRepoInfo {
        head: GitHead::Detached,
        ahead: 0,
        behind: 0,
        total_changed: 1,
        staged: 0,
        untracked: 0,
    };
    assert!(info.is_dirty());
}

#[test]
fn status_priority_ordering() {
    assert!(status_priority(GitStatus::Modified) > status_priority(GitStatus::New));
    assert!(status_priority(GitStatus::New) > status_priority(GitStatus::Deleted));
    assert!(status_priority(GitStatus::Deleted) > status_priority(GitStatus::Ignored));
}

#[test]
fn set_if_higher_inserts_new_key() {
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::New);
    assert!(matches!(map[&PathBuf::from("a.rs")], GitStatus::New));
}

#[test]
fn set_if_higher_upgrades_to_higher_priority() {
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::Deleted);
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::Modified);
    assert!(matches!(map[&PathBuf::from("a.rs")], GitStatus::Modified));
}

#[test]
fn set_if_higher_does_not_downgrade() {
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::Modified);
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::Ignored);
    assert!(matches!(map[&PathBuf::from("a.rs")], GitStatus::Modified));
}

#[test]
fn set_if_higher_equal_priority_unchanged() {
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::New);
    set_if_higher(&mut map, PathBuf::from("a.rs"), GitStatus::New);
    assert!(matches!(map[&PathBuf::from("a.rs")], GitStatus::New));
}
