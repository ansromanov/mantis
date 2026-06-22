use std::fs;
use std::path::Path;
use std::process::Command;

use super::*;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {:?} failed", args);
}

fn clone_repo(src: &Path, dst: &Path) {
    let status = Command::new("git")
        .arg("clone")
        .arg("-q")
        .arg(src)
        .arg(dst)
        .status()
        .unwrap();
    assert!(status.success(), "git clone {:?} -> {:?} failed", src, dst);
}

#[test]
fn single_line_single_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-mail <alice@example.com>\n\
         author-time 1000000\n\
         author-tz +0000\n\
         committer Alice\n\
         committer-mail <alice@example.com>\n\
         committer-time 1000000\n\
         committer-tz +0000\n\
         summary init\n\
         filename src/foo.rs\n\
         \tfn main() {}\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].short_hash, "aaaaaaa");
    assert_eq!(
        result[0].commit_hash,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn multi_line_same_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 3\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].line_no, 3);
    for b in &result {
        assert_eq!(b.author, "Alice");
    }
}

#[test]
fn multiple_commits_interleaved() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2 2 1\n\
         author Bob\n\
         author-time 2000000\n\
         filename src/foo.rs\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].author, "Bob");
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].author, "Alice");
    assert_eq!(result[2].line_no, 3);
}

#[test]
fn empty_input() {
    assert!(parse_blame_porcelain("").is_empty());
}

#[test]
fn ahead_behind_missing_upstream_is_none() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("a.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "a.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);

    assert_eq!(ahead_behind(dir.path()), None);
}

#[test]
fn ahead_behind_counts_against_upstream() {
    let origin = tempfile::tempdir().unwrap();
    git(
        origin.path(),
        &["init", "-q", "--bare", "--initial-branch=master"],
    );

    let local = tempfile::tempdir().unwrap();
    clone_repo(origin.path(), local.path());
    fs::write(local.path().join("a.txt"), "base\n").unwrap();
    git(local.path(), &["add", "a.txt"]);
    git(local.path(), &["commit", "-q", "-m", "base"]);
    git(local.path(), &["push", "-q", "-u", "origin", "master"]);

    let other = tempfile::tempdir().unwrap();
    clone_repo(origin.path(), other.path());
    fs::write(other.path().join("a.txt"), "base\nremote\n").unwrap();
    git(other.path(), &["commit", "-q", "-am", "remote"]);
    git(other.path(), &["push", "-q"]);

    git(local.path(), &["fetch", "-q"]);
    fs::write(local.path().join("a.txt"), "base\nlocal\n").unwrap();
    git(local.path(), &["commit", "-q", "-am", "local"]);

    assert_eq!(ahead_behind(local.path()), Some((1, 1)));
}

// -- subject field -----------------------------------------------------------

#[test]
fn blame_porcelain_parses_subject() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         summary fix the thing\n\
         filename src/foo.rs\n\
         \tcode here\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].subject, "fix the thing");
}

#[test]
fn blame_porcelain_subject_shared_across_lines_same_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 2\n\
         author Alice\n\
         author-time 1000000\n\
         summary shared subject\n\
         filename src/foo.rs\n\
         \tline one\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2\n\
         \tline two\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].subject, "shared subject");
    assert_eq!(result[1].subject, "shared subject");
}

#[test]
fn blame_porcelain_missing_summary_gives_empty_subject() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tcode here\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].subject, "");
}
