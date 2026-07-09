use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

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

// -- repo_status ------------------------------------------------------------

fn init_repo_with_gitignore(dir: &Path) {
    git(dir, &["init", "-q"]);
    // Tracked file.
    fs::write(dir.join("tracked.txt"), "hello\n").unwrap();
    git(dir, &["add", "tracked.txt"]);
    git(dir, &["commit", "-q", "-m", "init"]);
    // Untracked file.
    fs::write(dir.join("untracked.txt"), "new\n").unwrap();
    // Ignored file.
    fs::write(dir.join(".gitignore"), "*.log\n").unwrap();
    git(dir, &["add", ".gitignore"]);
    git(dir, &["commit", "-q", "-m", "add gitignore"]);
    fs::write(dir.join("build.log"), "log\n").unwrap();
}

/// Canonicalized root that `repo_status` uses internally. tempdir returns
/// paths via `/tmp` which on macOS is a symlink to `/private/tmp`; the
/// function's internal `git_toplevel` canonicalizes, so we must compare
/// against canonical paths.
fn repo_root(dir: &Path) -> PathBuf {
    dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf())
}

#[test]
fn repo_status_default_untracked_shown_ignored_hidden() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_gitignore(dir.path());
    let map = repo_status(dir.path(), true, false);
    let root = repo_root(dir.path());
    assert!(
        map.contains_key(&root.join("untracked.txt")),
        "untracked should be included by default"
    );
    assert_eq!(map.get(&root.join("untracked.txt")), Some(&GitStatus::New));
    assert!(
        !map.contains_key(&root.join("build.log")),
        "ignored should be excluded by default"
    );
}

#[test]
fn repo_status_untracked_excluded() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_gitignore(dir.path());
    let map = repo_status(dir.path(), false, false);
    let root = repo_root(dir.path());
    assert!(
        !map.contains_key(&root.join("untracked.txt")),
        "untracked should be excluded when include_untracked is false"
    );
}

#[test]
fn repo_status_ignored_included() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_gitignore(dir.path());
    let map = repo_status(dir.path(), true, true);
    let root = repo_root(dir.path());
    assert!(
        map.contains_key(&root.join("build.log")),
        "ignored should be included when include_ignored is true"
    );
    assert_eq!(map.get(&root.join("build.log")), Some(&GitStatus::Ignored));
}

#[test]
fn repo_status_both_excluded() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_gitignore(dir.path());
    let map = repo_status(dir.path(), false, false);
    let root = repo_root(dir.path());
    assert!(
        !map.contains_key(&root.join("untracked.txt")),
        "untracked excluded"
    );
    assert!(
        !map.contains_key(&root.join("build.log")),
        "ignored excluded"
    );
    // Tracked file is not modified, so map should be empty.
    for (path, status) in &map {
        assert_ne!(
            path.file_name().map(|n| n.to_string_lossy()),
            Some("tracked.txt".into()),
            "tracked.txt should not appear (it's unchanged): {status:?}"
        );
    }
}

// -- non-ASCII filenames (the -z fix) ---------------------------------------

#[test]
fn repo_status_non_ascii_filename() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("untracked.txt"), "hello\n").unwrap();
    // Non-ASCII UTF-8 name — git C-quotes it without -z.
    fs::write(dir.path().join("файл.txt"), "привет\n").unwrap();
    let map = repo_status(dir.path(), true, false);
    let root = repo_root(dir.path());
    assert!(
        map.contains_key(&root.join("файл.txt")),
        "non-ASCII filename should appear in status"
    );
    assert!(
        map.contains_key(&root.join("untracked.txt")),
        "ASCII filename should still appear"
    );
}

#[test]
fn repo_status_filename_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("prefix.txt"), "data\n").unwrap();
    // File with leading and trailing spaces (legal on Unix).
    let spaced_name = "  spaced-file.txt  ";
    fs::write(dir.path().join(spaced_name), "data\n").unwrap();
    let map = repo_status(dir.path(), true, false);
    let root = repo_root(dir.path());
    assert!(
        map.contains_key(&root.join("prefix.txt")),
        "normal filename should appear"
    );
    assert!(
        map.contains_key(&root.join(spaced_name)),
        "filename with spaces should appear"
    );
}

#[test]
fn repo_status_rename_is_tracked() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("original.txt"), "content\n").unwrap();
    git(dir.path(), &["add", "original.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);
    // Rename the file (staged).
    let new_name = "renamed.txt";
    // `git mv` stages the rename.
    git(dir.path(), &["mv", "original.txt", new_name]);
    let map = repo_status(dir.path(), true, false);
    let root = repo_root(dir.path());
    assert!(
        !map.contains_key(&root.join("original.txt")),
        "original name should not appear (it's renamed away)"
    );
    assert!(
        map.contains_key(&root.join(new_name)),
        "renamed file should appear in status"
    );
    assert_eq!(
        map.get(&root.join(new_name)),
        Some(&GitStatus::Modified),
        "staged rename should be Modified"
    );
}

// -- range_status (compare mode) --------------------------------------------

#[test]
fn range_status_reports_added_modified_deleted_renamed() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("modified.txt"), "v1\n").unwrap();
    fs::write(dir.path().join("deleted.txt"), "bye\n").unwrap();
    fs::write(dir.path().join("original.txt"), "content\n").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    let base = "HEAD".to_string();

    fs::write(dir.path().join("modified.txt"), "v2\n").unwrap();
    fs::remove_file(dir.path().join("deleted.txt")).unwrap();
    fs::write(dir.path().join("added.txt"), "new\n").unwrap();
    git(dir.path(), &["mv", "original.txt", "renamed.txt"]);
    git(dir.path(), &["add", "-A"]);

    let map = range_status(dir.path(), &base).expect("range_status should succeed");
    let root = repo_root(dir.path());
    assert_eq!(
        map.get(&root.join("modified.txt")),
        Some(&GitStatus::Modified)
    );
    assert_eq!(
        map.get(&root.join("deleted.txt")),
        Some(&GitStatus::Deleted)
    );
    assert_eq!(map.get(&root.join("added.txt")), Some(&GitStatus::New));
    assert_eq!(
        map.get(&root.join("renamed.txt")),
        Some(&GitStatus::Renamed)
    );
    assert!(
        !map.contains_key(&root.join("original.txt")),
        "old path of a rename should not appear"
    );
}

#[test]
fn range_status_unknown_revision_is_err() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("tracked.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);

    let result = range_status(dir.path(), "not-a-real-revision");
    assert!(result.is_err(), "unknown revision should return Err");
}

#[test]
fn range_status_dash_prefixed_revision_does_not_leak_as_git_flag() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    fs::write(dir.path().join("tracked.txt"), "hello\n").unwrap();
    git(dir.path(), &["add", "tracked.txt"]);
    git(dir.path(), &["commit", "-q", "-m", "init"]);

    // A revision string starting with `-` must not be interpreted as a git
    // option (e.g. `--cached`); it should just fail to resolve as a revision.
    let result = range_status(dir.path(), "--cached");
    assert!(result.is_err());
}

/// Regression test for a parser desync: an unhandled status char (here `T`,
/// type-change) has its *own path* starting with a status letter (`M`). A
/// buggy parser that skips only the status segment (not the path segment)
/// for unhandled statuses would misread that path as a bogus new status
/// entry, consuming the real next entry's status char as its own "path" and
/// losing the real next entry entirely.
#[test]
fn range_status_unhandled_status_does_not_desync_following_entries() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    // Path starts with 'M' so a buggy parser misreads it as a bogus Modified entry.
    fs::write(dir.path().join("Mtypechange.txt"), "content\n").unwrap();
    fs::write(dir.path().join("Zreal.txt"), "v1\n").unwrap();
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "base"]);
    let base = "HEAD".to_string();

    // Type-change: regular file -> symlink (git reports status 'T').
    fs::remove_file(dir.path().join("Mtypechange.txt")).unwrap();
    std::os::unix::fs::symlink("Zreal.txt", dir.path().join("Mtypechange.txt")).unwrap();
    fs::write(dir.path().join("Zreal.txt"), "v2\n").unwrap();
    git(dir.path(), &["add", "-A"]);

    let map = range_status(dir.path(), &base).expect("range_status should succeed");
    let root = repo_root(dir.path());
    assert_eq!(
        map.get(&root.join("Zreal.txt")),
        Some(&GitStatus::Modified),
        "the real entry following an unhandled type-change status must still parse correctly"
    );
}

// -- blame cache (bounded LRU) -----------------------------------------------

#[test]
fn blame_cache_evicts_oldest_when_full() {
    let mut cache = BlameCache::new();
    assert_eq!(BLAME_CACHE_CAPACITY, 16);

    let paths: Vec<PathBuf> = (0..BLAME_CACHE_CAPACITY + 1)
        .map(|i| PathBuf::from(format!("/tmp/test_blame_cache_{i}.rs")))
        .collect();

    for (i, p) in paths.iter().enumerate() {
        cache.insert(
            p.clone(),
            CachedBlame {
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(i as u64),
                lines: Vec::new(),
            },
        );
    }

    // Cache should be at capacity.
    assert_eq!(cache.map.len(), BLAME_CACHE_CAPACITY);
    assert_eq!(cache.order.len(), BLAME_CACHE_CAPACITY);

    // The first-inserted entry should be evicted.
    assert!(
        !cache.map.contains_key(&paths[0]),
        "oldest entry should be evicted"
    );

    // The last (most recent) entry should survive.
    assert!(
        cache.map.contains_key(&paths[BLAME_CACHE_CAPACITY]),
        "most recent entry should survive"
    );
}

#[test]
fn blame_cache_re_get_promotes_entry() {
    let mut cache = BlameCache::new();

    let a = PathBuf::from("/tmp/a.rs");
    let b = PathBuf::from("/tmp/b.rs");
    let c = PathBuf::from("/tmp/c.rs");

    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH,
            lines: Vec::new(),
        },
    );
    cache.insert(
        b.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            lines: Vec::new(),
        },
    );
    cache.insert(
        c.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
            lines: Vec::new(),
        },
    );

    // Order is [a, b, c]. Get 'a' to promote it.
    let _ = cache.get(&a);

    // Now order should be [b, c, a].
    assert_eq!(cache.order[0], b);
    assert_eq!(cache.order[1], c);
    assert_eq!(cache.order[2], a);
}

#[test]
fn blame_cache_update_promotes_entry() {
    let mut cache = BlameCache::new();

    let a = PathBuf::from("/tmp/a.rs");
    let b = PathBuf::from("/tmp/b.rs");

    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH,
            lines: Vec::new(),
        },
    );
    cache.insert(
        b.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            lines: Vec::new(),
        },
    );

    // Re-insert 'a' as if its mtime changed.
    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            lines: Vec::new(),
        },
    );

    // 'a' should be promoted to the back (most recent).
    assert_eq!(cache.order[0], b);
    assert_eq!(cache.order[1], a);
}

fn init_repo_with_commit(dir: &Path) {
    git(dir, &["init"]);
    let f = dir.join("f.txt");
    fs::write(&f, "hello\n").unwrap();
    git(dir, &["add", "f.txt"]);
    git(dir, &["commit", "-m", "initial commit"]);
}

#[test]
fn git_cmd_has_optional_locks_disabled() {
    let cmd = git_cmd();
    let envs: std::collections::HashMap<_, _> = cmd.get_envs().collect();
    let lock_val = envs.get(std::ffi::OsStr::new("GIT_OPTIONAL_LOCKS"));
    assert_eq!(lock_val, Some(&Some(std::ffi::OsStr::new("0"))));
}

#[test]
fn branches_returns_local_branches() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_commit(dir.path());
    let bs = branches(dir.path());
    assert!(
        !bs.is_empty(),
        "should have at least one branch after a commit"
    );
}

#[test]
fn branches_returns_empty_for_non_repo() {
    let dir = tempfile::tempdir().unwrap();
    let bs = branches(dir.path());
    assert!(bs.is_empty());
}

#[test]
fn tags_returns_empty_for_non_repo() {
    let dir = tempfile::tempdir().unwrap();
    let ts = tags(dir.path());
    assert!(ts.is_empty());
}

#[test]
fn recent_commits_returns_commits() {
    let dir = tempfile::tempdir().unwrap();
    init_repo_with_commit(dir.path());
    let commits = recent_commits(dir.path(), 10);
    assert_eq!(commits.len(), 1);
    assert_eq!(commits[0].subject, "initial commit");
}

#[test]
fn recent_commits_returns_empty_for_non_repo() {
    let dir = tempfile::tempdir().unwrap();
    let commits = recent_commits(dir.path(), 10);
    assert!(commits.is_empty());
}
