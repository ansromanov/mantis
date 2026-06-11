use std::path::Path;
use std::process::Command;

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
