use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use mantis::git::{file_diff, file_log, staged_diff, unstaged_diff, working_tree_diff};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=test@example.com", "-c", "user.name=Test"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn temp_repo() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_git_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    fs::write(dir.join("file.txt"), "one\n").unwrap();
    git(&dir, &["add", "file.txt"]);
    git(&dir, &["commit", "-q", "-m", "first"]);
    fs::write(dir.join("file.txt"), "one\ntwo\n").unwrap();
    git(&dir, &["add", "file.txt"]);
    git(&dir, &["commit", "-q", "-m", "second"]);
    fs::write(dir.join("file.txt"), "one\ntwo\nthree\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn file_log_returns_commits_newest_first() {
    let repo = temp_repo();
    let log = file_log(&repo, &repo.join("file.txt"));
    assert_eq!(log.len(), 2);
    assert_eq!(log[0].subject, "second");
    assert_eq!(log[1].subject, "first");
    assert!(!log[0].short.is_empty());
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn file_diff_against_working_tree() {
    let repo = temp_repo();
    let log = file_log(&repo, &repo.join("file.txt"));
    let diff = file_diff(&repo, &log[1].hash, &repo.join("file.txt"));
    let joined = diff.join("\n");
    assert!(joined.contains("+two"), "diff was: {joined}");
    assert!(joined.contains("+three"), "diff was: {joined}");
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn file_log_empty_outside_repo() {
    let dir = std::env::temp_dir();
    let log = file_log(&dir, Path::new("definitely-not-tracked-xyz.txt"));
    assert!(log.is_empty());
}

#[test]
fn working_tree_diff_shows_modifications() {
    let repo = temp_repo();
    let diff = working_tree_diff(&repo, &repo.join("file.txt"));
    let joined = diff.join("\n");
    assert!(
        joined.contains("+three"),
        "expected '+three' in diff, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn working_tree_diff_shows_untracked_file() {
    let repo = temp_repo();
    fs::write(repo.join("new.txt"), "brand new\n").unwrap();
    let diff = working_tree_diff(&repo, &repo.join("new.txt"));
    let joined = diff.join("\n");
    assert!(
        joined.contains("+brand new"),
        "expected '+brand new' in diff, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn staged_diff_shows_only_staged_changes() {
    let repo = temp_repo();
    // Stage the working-tree change.
    git(&repo, &["add", "file.txt"]);
    let diff = staged_diff(&repo, &repo.join("file.txt"));
    let joined = diff.join("\n");
    assert!(
        joined.contains("+three"),
        "staged diff should include the staged addition, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn staged_diff_empty_when_nothing_staged() {
    let repo = temp_repo();
    // file.txt has an unstaged change but nothing is staged.
    let diff = staged_diff(&repo, &repo.join("file.txt"));
    // No staged hunk → placeholder message, not an actual diff.
    assert!(
        diff.iter()
            .any(|l| !l.starts_with('+') && !l.starts_with('-')),
        "expected a placeholder, not a real diff header, got: {diff:?}"
    );
    let joined = diff.join("\n");
    assert!(
        !joined.contains("@@"),
        "no staged changes should mean no hunk headers, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn unstaged_diff_shows_only_unstaged_changes() {
    let repo = temp_repo();
    // Nothing is staged: the modification is purely in the working tree.
    let diff = unstaged_diff(&repo, &repo.join("file.txt"));
    let joined = diff.join("\n");
    assert!(
        joined.contains("+three"),
        "unstaged diff should include the working-tree addition, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}

#[test]
fn unstaged_diff_empty_after_staging() {
    let repo = temp_repo();
    // Stage everything: nothing remains unstaged.
    git(&repo, &["add", "file.txt"]);
    let diff = unstaged_diff(&repo, &repo.join("file.txt"));
    let joined = diff.join("\n");
    assert!(
        !joined.contains("@@"),
        "after staging all changes there should be no unstaged hunk headers, got: {joined}"
    );
    fs::remove_dir_all(&repo).ok();
}
