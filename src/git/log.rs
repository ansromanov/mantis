//! Commit history (per-file and repo-wide), branches and tags.

use std::path::Path;
use std::process::Command;

use super::core::{git_cmd, git_toplevel};
use super::types::*;

/// Returns the commit history for a single file, newest first. Empty if the
/// file is untracked, not in a git repository, or git is unavailable.
pub fn file_log(repo_dir: &Path, file: &Path) -> Vec<Commit> {
    let output = git_cmd()
        .arg("-C")
        .arg(repo_dir)
        .args([
            "log",
            "--no-color",
            "--date=short",
            "--format=%H%x1f%h%x1f%ad%x1f%an%x1f%s",
            "--",
        ])
        .arg(file)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Commit {
                hash: parts.next()?.to_string(),
                short: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                author: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Lists recent commits from all branches, newest first.
/// Returns up to `count` commits. Empty when not in a git repo.
pub fn recent_commits(dir: &Path, count: usize) -> Vec<Commit> {
    let Some(root) = git_toplevel(dir) else {
        return Vec::new();
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args([
            "log",
            "--all",
            &format!("--max-count={}", count),
            "--format=%H%x1f%h%x1f%ad%x1f%an%x1f%s",
        ])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Commit {
                hash: parts.next()?.to_string(),
                short: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                author: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Returns the repository-wide commit log, newest first, with paged loading.
///
/// Fetches `limit` commits starting after `skip` already-loaded ones. Use
/// `skip = 0, limit = 200` for the initial load, then increment `skip` by
/// `limit` when the user scrolls past the end. Returns an empty vec when
/// not in a git repo or git is unavailable.
pub fn repo_log(dir: &Path, skip: usize, limit: usize) -> Vec<Commit> {
    let Some(root) = git_toplevel(dir) else {
        return Vec::new();
    };
    let mut cmd = Command::new("git");
    cmd.env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(&root)
        .args([
            "log",
            "--all",
            &format!("--skip={skip}"),
            &format!("--max-count={limit}"),
            "--format=%H%x1f%h%x1f%ad%x1f%an%x1f%s",
        ]);
    let output = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\u{1f}');
            Some(Commit {
                hash: parts.next()?.to_string(),
                short: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                author: parts.next().unwrap_or("").to_string(),
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Lists all local branch names. Empty when not in a git repo.
pub fn branches(dir: &Path) -> Vec<String> {
    let Some(root) = git_toplevel(dir) else {
        return Vec::new();
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["branch", "--format=%(refname:short)"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

/// Lists all tag names, newest first (by creation date). Empty when not in a
/// git repo.
pub fn tags(dir: &Path) -> Vec<String> {
    let Some(root) = git_toplevel(dir) else {
        return Vec::new();
    };
    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["tag", "--sort=-creatordate"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}
