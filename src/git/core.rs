//! Git integration via the `git` CLI.
//!
//! Rather than linking a Rust git library, this module shells out to `git` for
//! everything it needs: per-file working-tree status (`repo_status`), repository
//! metadata such as branch/HEAD and upstream counts (`repo_info`/`GitRepoInfo`
//! and `ahead_behind`), commit history, and unified diffs (working-tree and
//! per-commit). Results are cached behind a mutex keyed on a coarse timestamp so
//! rapid redraws don't spawn a process per frame.
//!
//! Every call degrades gracefully - a missing repo, a `git` that isn't
//! installed, or a failed command yields empty/`None` results instead of an
//! error, so the viewer works fine outside a repository.
//!
//! **Feature gate:** This module is compiled only when the `git-core` feature
//! is enabled (default on). When disabled, the same functionality is provided
//! by the bundled `git-plugin` subprocess plugin. Types are in the sibling `types` module
//! and are always available regardless of the feature flag.
//!
//! Three diff helpers cover the main diff modes available in the content pane:
//! `working_tree_diff` (all changes vs HEAD), `staged_diff` (index vs HEAD),
//! and `unstaged_diff` (worktree vs index).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::*;

/// Returns the git repository root containing `dir`, canonicalized.
fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    PathBuf::from(s).canonicalize().ok()
}

/// Returns rich git repository info for the directory containing `dir`, or
/// `None` if not in a git repo or git is unavailable.
pub fn repo_info(dir: &Path) -> Option<GitRepoInfo> {
    let root = git_toplevel(dir)?;

    let out = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain", "-b"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut info = parse_repo_info(&text);
    // The porcelain branch header includes "..." only when a tracking upstream
    // exists ("## branch...upstream [ahead N]"). Skip the extra rev-list call
    // entirely for detached HEADs and untracked branches.
    let has_upstream = text
        .lines()
        .next()
        .and_then(|l| l.strip_prefix("## "))
        .is_some_and(|l| l.contains("..."));
    if has_upstream {
        if let Some((ahead, behind)) = ahead_behind(&root) {
            info.ahead = ahead;
            info.behind = behind;
        }
    }

    // Refine HEAD state: check for rebase/merge by inspecting git state files.
    let git_dir = root.join(".git");
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        info.head = GitHead::Rebase;
    } else if git_dir.join("MERGE_HEAD").exists() {
        info.head = GitHead::Merge;
    }

    Some(info)
}

/// Returns how many commits `HEAD` is ahead of and behind its upstream.
///
/// Missing upstreams and git errors return `None` so callers can omit the
/// indicator entirely.
pub fn ahead_behind(repo_dir: &Path) -> Option<(usize, usize)> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-list", "--left-right", "--count", "HEAD...@{u}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let ahead = parts.next()?.parse().ok()?;
    let behind = parts.next()?.parse().ok()?;
    Some((ahead, behind))
}

fn parse_branch_line(line: &str) -> GitHead {
    let line = match line.strip_prefix("## ") {
        Some(l) => l,
        None => return GitHead::default(),
    };

    if line.starts_with("HEAD (no branch)")
        || line.starts_with("Initial commit on ")
        || line.starts_with("No commits yet on ")
    {
        return GitHead::Detached;
    }

    let branch = if let Some(pos) = line.find("...") {
        &line[..pos]
    } else if let Some(pos) = line.find(" [") {
        &line[..pos]
    } else {
        line
    };
    GitHead::Branch(branch.to_string())
}

fn parse_repo_info(text: &str) -> GitRepoInfo {
    let mut lines = text.lines();

    let branch_line = lines.next().unwrap_or("");
    let head = parse_branch_line(branch_line);

    let mut info = GitRepoInfo {
        head,
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    };

    for line in lines {
        if line.len() < 2 {
            continue;
        }
        let bytes = line.as_bytes();
        let x = bytes[0] as char;
        let y = bytes[1] as char;

        if x == '!' && y == '!' {
            continue;
        }

        info.total_changed += 1;

        if x == '?' && y == '?' {
            info.untracked += 1;
        } else if x != ' ' {
            info.staged += 1;
        }
    }

    info
}

/// Builds an absolute-path → status map for the repository containing `dir`.
/// Parent directories are included with their highest-priority child status so
/// collapsed dirs can be colored when they contain changes.
pub fn repo_status(
    dir: &Path,
    include_untracked: bool,
    include_ignored: bool,
) -> HashMap<PathBuf, GitStatus> {
    let Some(root) = git_toplevel(dir) else {
        return HashMap::new();
    };

    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(&root).arg("status").arg("--porcelain");
    if include_ignored {
        cmd.arg("--ignored");
    }
    let out = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return HashMap::new(),
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();

    for line in text.lines() {
        if line.len() < 3 {
            continue;
        }
        let x = line.as_bytes()[0] as char;
        let y = line.as_bytes()[1] as char;
        let path_str = line[3..].trim();
        let path_str = path_str
            .find(" -> ")
            .map_or(path_str, |i| &path_str[i + 4..]);
        let path_str = path_str.trim_end_matches('/');
        if path_str.is_empty() {
            continue;
        }

        if x == '?' && y == '?' && !include_untracked {
            continue;
        }
        if x == '!' && y == '!' && !include_ignored {
            continue;
        }

        let status = if x == '!' && y == '!' {
            GitStatus::Ignored
        } else if x == '?' && y == '?' {
            GitStatus::New
        } else if x == 'D' || y == 'D' {
            GitStatus::Deleted
        } else if x == 'A' && y == ' ' {
            GitStatus::New
        } else {
            GitStatus::Modified
        };

        let abs = root.join(path_str);
        set_if_higher(&mut map, abs.clone(), status);

        if status != GitStatus::Ignored {
            let mut cur = abs.parent();
            while let Some(d) = cur {
                if d == root.as_path() || !d.starts_with(&root) {
                    break;
                }
                set_if_higher(&mut map, d.to_path_buf(), status);
                cur = d.parent();
            }
        }
    }

    map
}

/// Returns the working-tree diff for `file` compared to HEAD, as lines.
/// For new untracked files that aren't staged, falls back to a
/// `--no-index` diff against `/dev/null`.
pub fn working_tree_diff(repo_dir: &Path, file: &Path) -> Vec<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "HEAD", "--no-color", "--"])
        .arg(file)
        .output();

    if let Ok(o) = out {
        let text = String::from_utf8_lossy(&o.stdout);
        if !text.trim().is_empty() {
            return text.lines().map(|l| l.to_string()).collect();
        }
    }

    // Untracked (unstaged) new file: build the diff manually.
    let bytes = match std::fs::read(file) {
        Ok(b) => b,
        Err(_) => return vec!["(no diff available)".to_string()],
    };
    if bytes.contains(&0u8) {
        let rel = file
            .strip_prefix(repo_dir)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        return vec![format!("Binary file {rel} added")];
    }
    let text = String::from_utf8_lossy(&bytes);
    let rel = file
        .strip_prefix(repo_dir)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/");
    let line_count = text.lines().count();
    let mut lines = vec![
        format!("diff --git a/{rel} b/{rel}"),
        "new file mode 100644".to_string(),
        "--- /dev/null".to_string(),
        format!("+++ b/{rel}"),
        format!("@@ -0,0 +1,{line_count} @@"),
    ];
    for line in text.lines() {
        lines.push(format!("+{line}"));
    }
    lines
}

/// Returns the staged diff for `file` (index vs. HEAD), as lines.
/// Returns a placeholder message when there are no staged changes.
pub fn staged_diff(repo_dir: &Path, file: &Path) -> Vec<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "--cached", "--no-color", "--"])
        .arg(file)
        .output();
    match out {
        Ok(o) if !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        _ => vec!["(no staged changes)".to_string()],
    }
}

/// Returns the unstaged diff for `file` (worktree vs. index), as lines.
/// Returns a placeholder message when there are no unstaged changes.
pub fn unstaged_diff(repo_dir: &Path, file: &Path) -> Vec<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "--no-color", "--"])
        .arg(file)
        .output();
    match out {
        Ok(o) if !o.stdout.is_empty() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect(),
        _ => vec!["(no unstaged changes)".to_string()],
    }
}
/// Returns the commit history for a single file, newest first. Empty if the
/// file is untracked, not in a git repository, or git is unavailable.
pub fn file_log(repo_dir: &Path, file: &Path) -> Vec<Commit> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args([
            "log",
            "--no-color",
            "--date=short",
            "--format=%H%x1f%h%x1f%ad%x1f%s",
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
                subject: parts.next().unwrap_or("").to_string(),
            })
        })
        .collect()
}

/// Returns the diff of `file` between `rev` and the current working tree, as
/// lines. On error or git being unavailable, returns a single message line.
pub fn file_diff(repo_dir: &Path, rev: &str, file: &Path) -> Vec<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "--no-color", rev, "--"])
        .arg(file)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.trim().is_empty() {
                vec!["(no changes between this revision and the working tree)".to_string()]
            } else {
                text.lines().map(|l| l.to_string()).collect()
            }
        }
        Ok(o) => vec![format!(
            "[git error] {}",
            String::from_utf8_lossy(&o.stderr).trim()
        )],
        Err(e) => vec![format!("[git unavailable] {e}")],
    }
}

struct CachedBlame {
    mtime: SystemTime,
    lines: Vec<BlameLine>,
}

static BLAME_CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedBlame>>> = OnceLock::new();

fn blame_cache() -> &'static Mutex<HashMap<PathBuf, CachedBlame>> {
    BLAME_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns per-line git blame annotations for `file` in the repository at
/// `repo_dir`. Returns an empty `Vec` if the file is untracked, not in a git
/// repo, or git is unavailable.
pub fn file_blame(repo_dir: &Path, file: &Path) -> Vec<BlameLine> {
    let mtime = match std::fs::metadata(file).and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    {
        let guard = blame_cache().lock().unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = guard.get(file) {
            if cached.mtime == mtime {
                return cached.lines.clone();
            }
        }
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["blame", "--porcelain", "--"])
        .arg(file)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let text = String::from_utf8_lossy(&output.stdout);
    let lines = parse_blame_porcelain(&text);

    {
        let mut guard = blame_cache().lock().unwrap_or_else(|p| p.into_inner());
        guard.insert(
            file.to_path_buf(),
            CachedBlame {
                mtime,
                lines: lines.clone(),
            },
        );
    }

    lines
}

fn parse_blame_porcelain(text: &str) -> Vec<BlameLine> {
    let mut blames = Vec::new();
    let mut meta: HashMap<String, (String, String, String)> = HashMap::new();
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 3 {
            continue;
        }
        let hash = parts[0].to_string();
        let line_no: u32 = parts[2].parse().unwrap_or(0);

        let (author, date_relative, subject) = if let Some(m) = meta.get(&hash) {
            (m.0.clone(), m.1.clone(), m.2.clone())
        } else {
            let mut author = String::from("Unknown");
            let mut author_time: u64 = 0;
            let mut subject = String::new();

            loop {
                match lines.next() {
                    Some(meta_line) if meta_line.starts_with("author ") => {
                        author = meta_line["author ".len()..].to_string();
                    }
                    Some(meta_line) if meta_line.starts_with("author-time ") => {
                        author_time = meta_line["author-time ".len()..].parse().unwrap_or(0);
                    }
                    Some(meta_line) if meta_line.starts_with("summary ") => {
                        subject = meta_line["summary ".len()..].to_string();
                    }
                    Some(meta_line) if meta_line.starts_with("filename ") => break,
                    Some(_) => continue,
                    None => break,
                }
            }

            let date_relative = format_relative_time(author_time);
            meta.insert(
                hash.clone(),
                (author.clone(), date_relative.clone(), subject.clone()),
            );
            (author, date_relative, subject)
        };

        if lines.next().is_some() {
            blames.push(BlameLine {
                short_hash: if hash.len() >= 7 {
                    hash[..7].to_string()
                } else {
                    hash.clone()
                },
                commit_hash: hash,
                author,
                date_relative,
                line_no,
                subject,
            });
        }
    }

    blames
}

fn format_relative_time(unix_ts: u64) -> String {
    if unix_ts == 0 {
        return "Not committed yet".to_string();
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let diff = now.saturating_sub(unix_ts);

    if diff < 60 {
        let n = diff.max(1);
        pluralize(n, "second")
    } else if diff < 3600 {
        pluralize(diff / 60, "minute")
    } else if diff < 86400 {
        pluralize(diff / 3600, "hour")
    } else if diff < 604800 {
        pluralize(diff / 86400, "day")
    } else if diff < 2_592_000 {
        pluralize(diff / 604800, "week")
    } else if diff < 31_536_000 {
        pluralize(diff / 2_592_000, "month")
    } else {
        pluralize(diff / 31_536_000, "year")
    }
}

fn pluralize(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
#[path = "core_test.rs"]
mod tests;
