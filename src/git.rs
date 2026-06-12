use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Per-file git working-tree status.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    New,
    Modified,
    Deleted,
    Ignored,
}

fn status_priority(s: GitStatus) -> u8 {
    match s {
        GitStatus::Modified => 3,
        GitStatus::New => 2,
        GitStatus::Deleted => 1,
        GitStatus::Ignored => 0,
    }
}

fn set_if_higher(map: &mut HashMap<PathBuf, GitStatus>, key: PathBuf, val: GitStatus) {
    let cur = map.entry(key).or_insert(val);
    if status_priority(val) > status_priority(*cur) {
        *cur = val;
    }
}

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

/// Builds an absolute-path → status map for the repository containing `dir`.
/// Parent directories are included with their highest-priority child status so
/// collapsed dirs can be colored when they contain changes.
/// Pass `include_ignored = true` only when the tree is showing gitignored files;
/// omitting `--ignored` makes the status call significantly faster on large repos.
pub fn repo_status(dir: &Path, include_ignored: bool) -> HashMap<PathBuf, GitStatus> {
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
        // Renames: "old -> new" — keep the destination path.
        let path_str = path_str
            .find(" -> ")
            .map_or(path_str, |i| &path_str[i + 4..]);
        // Ignored directories are listed with a trailing slash.
        let path_str = path_str.trim_end_matches('/');
        if path_str.is_empty() {
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

        // Propagate up through parent directories, but never for Ignored — doing
        // so would incorrectly taint the parent with the ignored status.
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

    // Untracked (unstaged) new file — diff against /dev/null.
    let out = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "--no-color", "--no-index", "--", "/dev/null"])
        .arg(file)
        .output();

    match out {
        Ok(o) => {
            let text = String::from_utf8_lossy(&o.stdout);
            if !text.trim().is_empty() {
                text.lines().map(|l| l.to_string()).collect()
            } else {
                vec!["(no diff available)".to_string()]
            }
        }
        Err(e) => vec![format!("[git unavailable] {e}")],
    }
}

/// A commit that touched a particular file.
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub date: String,
    pub subject: String,
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
            // hash, short hash, date, subject — separated by unit-separator
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(dir)
            // Keep the test independent of the user's global git config.
            .args(["-c", "user.email=test@example.com", "-c", "user.name=Test"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    /// A temp repo with two commits to `file.txt` plus an uncommitted change.
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
        // Uncommitted working-tree change.
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
        // Diff from the first commit to the working tree adds "two" and "three".
        let diff = file_diff(&repo, &log[1].hash, &repo.join("file.txt"));
        let joined = diff.join("\n");
        assert!(joined.contains("+two"), "diff was: {joined}");
        assert!(joined.contains("+three"), "diff was: {joined}");
        fs::remove_dir_all(&repo).ok();
    }

    #[test]
    fn file_log_empty_outside_repo() {
        let dir = std::env::temp_dir();
        // A path with no repo / untracked yields no history rather than erroring.
        let log = file_log(&dir, Path::new("definitely-not-tracked-xyz.txt"));
        assert!(log.is_empty());
    }
}
