//! Working-tree, staged and per-revision diffs, plus file contents at a revision.

use std::path::Path;

use super::core::{git_cmd, git_toplevel};

/// Returns the working-tree diff for `file` compared to HEAD, as lines.
/// For new untracked files that aren't staged, falls back to a
/// `--no-index` diff against `/dev/null`.
pub fn working_tree_diff(repo_dir: &Path, file: &Path) -> Vec<String> {
    let out = git_cmd()
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
    let out = git_cmd()
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
    let out = git_cmd()
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

/// Returns the content of `file` as it was at revision `rev`, via `git show
/// <rev>:<relpath>`. Returns `None` when the file does not exist at that
/// revision, the path is not relative to the repo root, or git is unavailable.
pub fn file_at_rev(repo_dir: &Path, rev: &str, file: &Path) -> Option<Vec<u8>> {
    let root = git_toplevel(repo_dir)?;
    let file = file.canonicalize().ok()?;
    let rel = file.strip_prefix(&root).ok()?;
    let rel_str = rel.to_str()?;
    let spec = format!("{rev}:{rel_str}");
    let out = git_cmd()
        .arg("-C")
        .arg(&root)
        .args(["show", &spec])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(out.stdout)
}

/// Returns the diff of `file` between `rev` and the current working tree, as
/// lines. On error or git being unavailable, returns a single message line.
pub fn file_diff(repo_dir: &Path, rev: &str, file: &Path) -> Vec<String> {
    let output = git_cmd()
        .arg("-C")
        .arg(repo_dir)
        .args(["diff", "--no-color", "--end-of-options", rev, "--"])
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

/// Returns the patch introduced by `rev` for `file`, compared with its parent.
/// This is the appropriate view for a file-history entry; unlike `file_diff`,
/// it does not compare the selected commit with the current working tree.
pub fn file_commit_diff(repo_dir: &Path, rev: &str, file: &Path) -> Vec<String> {
    let output = git_cmd()
        .arg("-C")
        .arg(repo_dir)
        .args([
            "show",
            "--format=",
            "--no-color",
            "--end-of-options",
            rev,
            "--",
        ])
        .arg(file)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.trim().is_empty() {
                vec!["(no changes in this commit for the selected file)".to_string()]
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
