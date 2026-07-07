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
    assert!(status_priority(GitStatus::Renamed) > status_priority(GitStatus::Modified));
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

// -- BlameLine subject field -------------------------------------------------

#[test]
fn blame_line_subject_empty_by_default_construction() {
    let b = BlameLine {
        commit_hash: "a".repeat(40),
        short_hash: "aaaaaaa".to_string(),
        author: "Alice".to_string(),
        date_relative: "2 days ago".to_string(),
        line_no: 1,
        subject: String::new(),
    };
    assert_eq!(b.subject, "");
}

#[test]
fn blame_line_subject_roundtrips() {
    let msg = "add tests for blame popup".to_string();
    let b = BlameLine {
        commit_hash: "b".repeat(40),
        short_hash: "bbbbbbb".to_string(),
        author: "Bob".to_string(),
        date_relative: "1 hour ago".to_string(),
        line_no: 5,
        subject: msg.clone(),
    };
    assert_eq!(b.subject, msg);
}

// -- GitStatus Debug derive --------------------------------------------------

#[test]
fn git_status_derives_debug() {
    assert_eq!(format!("{:?}", GitStatus::New), "New");
    assert_eq!(format!("{:?}", GitStatus::Modified), "Modified");
    assert_eq!(format!("{:?}", GitStatus::Deleted), "Deleted");
    assert_eq!(format!("{:?}", GitStatus::Ignored), "Ignored");
    assert_eq!(format!("{:?}", GitStatus::Renamed), "Renamed");
}
