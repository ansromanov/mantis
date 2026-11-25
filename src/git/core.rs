//! Git integration via the `git` CLI.
//!
//! Rather than linking a Rust git library, this module shells out to `git` for
//! everything it needs: per-file working-tree status (`repo_status`), repository
//! metadata such as branch/HEAD and upstream counts (`repo_info`/`GitRepoInfo`
//! and `ahead_behind`), commit history, and unified diffs (working-tree and
//! per-commit). Results are cached behind a mutex keyed on a coarse timestamp so
//! rapid redraws don't spawn a process per frame.
//!
//! To prevent lock contention with concurrent user git commands, all read-only
//! invocations here disable optional locks by setting the `GIT_OPTIONAL_LOCKS=0`
//! environment variable, and status calls additionally pass `--no-optional-locks`.
//!
//! Every call degrades gracefully - a missing repo, a `git` that isn't
//! installed, or a failed command yields empty/`None` results instead of an
//! error, so the viewer works fine outside a repository.
//!
//! All git functionality is built in.
//!
//! Three diff helpers cover the main diff modes available in the content pane:
//! `working_tree_diff` (all changes vs HEAD), `staged_diff` (index vs HEAD),
//! and `unstaged_diff` (worktree vs index).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::*;

/// Helper to construct a `Command` running `git` with optional locks disabled.
///
/// Disabling optional locks (via `GIT_OPTIONAL_LOCKS=0`) ensures that read-only
/// operations (like `git status`) do not refresh the index and write to
/// `.git/index.lock`, preventing race conditions with concurrent user git
/// commands (such as a pull or commit).
pub(super) fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd
}

/// Returns the git repository root containing `dir`, canonicalized.
pub(super) fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    let out = git_cmd()
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

/// Verifies that `rev` resolves to a commit in the repository containing
/// `dir`. The returned error is suitable for showing in the status bar.
pub fn verify_revision(dir: &Path, rev: &str) -> Result<(), String> {
    let Some(root) = git_toplevel(dir) else {
        return Err("not a git repository".to_string());
    };
    let spec = format!("{rev}^{{commit}}");
    let output = git_cmd()
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "--end-of-options"])
        .arg(spec)
        .output()
        .map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "revision '{rev}' not found: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Returns the short hash for a verified revision.
pub fn short_revision(dir: &Path, rev: &str) -> Option<String> {
    let root = git_toplevel(dir)?;
    let spec = format!("{rev}^{{commit}}");
    let output = git_cmd()
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--short", "--end-of-options"])
        .arg(spec)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Returns rich git repository info for the directory containing `dir`, or
/// `None` if not in a git repo or git is unavailable.
///
/// Note: this calls `git status --porcelain -b` **without** `-z` for two
/// reasons: (1) this function only reads branch/HEAD/count info, never
/// parses paths, so C-quoting is irrelevant, and (2) `-b` output is always
/// a single branch-header line followed by status entries, and the branch
/// header contains no quoted paths.
pub fn repo_info(dir: &Path) -> Option<GitRepoInfo> {
    let root = git_toplevel(dir)?;

    let out = git_cmd()
        .arg("--no-optional-locks")
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
    let out = git_cmd()
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

/// Lists worktrees known to the repository, preserving git's porcelain order.
pub fn worktree_list(repo_dir: &Path) -> Vec<Worktree> {
    let out = git_cmd()
        .arg("-C")
        .arg(repo_dir)
        .args(["worktree", "list", "--porcelain"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    parse_worktree_list(&String::from_utf8_lossy(&out.stdout))
}

pub(crate) fn parse_worktree_list(text: &str) -> Vec<Worktree> {
    let mut result = Vec::new();
    let mut current: Option<Worktree> = None;
    for line in text.lines() {
        if line.is_empty() {
            if let Some(item) = current.take() {
                result.push(item);
            }
            continue;
        }
        let (key, value) = if let Some(rest) = line.strip_prefix("worktree ") {
            ("worktree", rest)
        } else {
            line.split_once(' ').unwrap_or((line, ""))
        };
        match key {
            "worktree" => {
                if let Some(item) = current.take() {
                    result.push(item);
                }
                current = Some(Worktree {
                    path: PathBuf::from(value),
                    head: String::new(),
                    branch: None,
                    locked: false,
                    prunable: false,
                });
            }
            "HEAD" => {
                if let Some(item) = current.as_mut() {
                    item.head = value.to_string();
                }
            }
            "branch" => {
                if let Some(item) = current.as_mut() {
                    item.branch = value.strip_prefix("refs/heads/").map(str::to_string);
                }
            }
            "detached" => {}
            "locked" => {
                if let Some(item) = current.as_mut() {
                    item.locked = true;
                }
            }
            "prunable" => {
                if let Some(item) = current.as_mut() {
                    item.prunable = true;
                }
            }
            _ => {}
        }
    }
    if let Some(item) = current {
        result.push(item);
    }
    result
}

fn parse_branch_line(line: &str) -> GitHead {
    let line = match line.strip_prefix("## ") {
        Some(l) => l,
        None => return GitHead::default(),
    };

    if line.starts_with("HEAD (no branch)") {
        return GitHead::Detached;
    }

    if let Some(branch) = line
        .strip_prefix("Initial commit on ")
        .or_else(|| line.strip_prefix("No commits yet on "))
    {
        return GitHead::Branch(branch.to_string());
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
///
/// Uses `git status --porcelain -z` so paths are never C-quoted (safe for
/// non-ASCII, spaces, quotes, and control characters).
#[allow(dead_code)]
pub fn repo_status(
    dir: &Path,
    include_untracked: bool,
    include_ignored: bool,
) -> HashMap<PathBuf, GitStatus> {
    repo_status_details(dir, include_untracked, include_ignored).status_map
}

/// Returns detailed git repository status for `dir`, recording per-file `GitStatus`
/// and partitioning changed files into staged, unstaged, and untracked sets.
pub fn repo_status_details(
    dir: &Path,
    include_untracked: bool,
    include_ignored: bool,
) -> GitStatusDetails {
    let Some(root) = git_toplevel(dir) else {
        return GitStatusDetails::default();
    };

    let mut cmd = git_cmd();
    cmd.arg("--no-optional-locks")
        .arg("-C")
        .arg(&root)
        .args(["status", "--porcelain", "-z"]);
    if include_ignored {
        cmd.arg("--ignored");
    }
    let out = match cmd.output() {
        Ok(o) if o.status.success() => o,
        _ => return GitStatusDetails::default(),
    };

    let mut details = GitStatusDetails::default();

    // With -z, each entry is NUL-terminated and paths are never C-quoted.
    //   Normal:  XY path\0
    //   Rename:  R  orig_path\0new_path\0
    let bytes = &out.stdout;
    let mut segs: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    // The final NUL produces a trailing empty segment; drop it.
    if segs.last().is_some_and(|s| s.is_empty()) {
        segs.pop();
    }

    let mut i = 0;
    while i < segs.len() {
        let seg = segs[i];
        if seg.len() < 3 {
            i += 1;
            continue;
        }

        let x = seg[0] as char;
        let y = seg[1] as char;
        let path_bytes: &[u8] = &seg[3..];

        // With -z, rename entries are R  dest\0src\0 — the destination
        // (new) path is at seg[3..] and the source (old) path is the next
        // NUL-delimited segment.  We only need the destination, so consume
        // the source segment here.
        if x == 'R' || y == 'R' {
            i += 1;
        }

        let path_str = std::str::from_utf8(path_bytes).unwrap_or("");
        let path_str = path_str.trim_end_matches('/');
        if path_str.is_empty() {
            i += 1;
            continue;
        }

        if x == '?' && y == '?' && !include_untracked {
            i += 1;
            continue;
        }
        if x == '!' && y == '!' && !include_ignored {
            i += 1;
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
        if x != ' ' && x != '?' && x != '!' {
            details.staged.insert(abs.clone());
        }
        if y != ' ' && y != '?' && y != '!' {
            details.unstaged.insert(abs.clone());
        }
        if x == '?' && y == '?' {
            details.untracked.insert(abs.clone());
        }

        set_if_higher(&mut details.status_map, abs.clone(), status);

        if status != GitStatus::Ignored {
            let mut cur = abs.parent();
            while let Some(d) = cur {
                if d == root.as_path() || !d.starts_with(&root) {
                    break;
                }
                set_if_higher(&mut details.status_map, d.to_path_buf(), status);
                cur = d.parent();
            }
        }

        i += 1;
    }

    details
}

/// Returns the set of files changed between `<rev>` and the working tree, with
/// their status (Added/Modified/Deleted/Renamed), using `git diff --name-status -z`.
///
/// The output with `-z` is a sequence of NUL-delimited fields alternating
/// between status and path: `M\0path\0`, `A\0path\0`, `D\0path\0`, or for
/// renames: `R<score>\0old_path\0new_path\0`.
///
/// Parent directories are included with their highest-priority child status so
/// collapsed dirs can be colored when they contain changes.
///
/// Returns `Err` with a message describing the failure (e.g. an unknown
/// revision) so the caller can surface it to the user instead of silently
/// showing an empty compare view.
pub fn range_status(dir: &Path, rev: &str) -> Result<HashMap<PathBuf, GitStatus>, String> {
    let Some(root) = git_toplevel(dir) else {
        return Err("not a git repository".to_string());
    };

    let out = git_cmd()
        .arg("-C")
        .arg(&root)
        .args(["diff", "--name-status", "-z", "--end-of-options", rev])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(o) => return Err(String::from_utf8_lossy(&o.stderr).trim().to_string()),
        Err(e) => return Err(e.to_string()),
    };

    Ok(parse_name_status(&root, &out.stdout))
}

/// Parses NUL-delimited git name-status output and adds parent directories.
fn parse_name_status(root: &Path, bytes: &[u8]) -> HashMap<PathBuf, GitStatus> {
    let mut map: HashMap<PathBuf, GitStatus> = HashMap::new();
    let mut segs: Vec<&[u8]> = bytes.split(|&b| b == 0).collect();
    if segs.last().is_some_and(|s| s.is_empty()) {
        segs.pop();
    }

    let mut i = 0;
    while i < segs.len() {
        let seg = segs[i];
        if seg.is_empty() {
            i += 1;
            continue;
        }

        // First character tells us the status.
        let status_char = seg[0] as char;
        let status = match status_char {
            'A' => GitStatus::New,
            'M' => GitStatus::Modified,
            'D' => GitStatus::Deleted,
            'R' => GitStatus::Renamed,
            'C' => GitStatus::New,
            _ => {
                // Unrecognised status (e.g. type-change 'T', unmerged 'U'):
                // skip both the status and its path segment so the stream
                // stays aligned for the next entry.
                i += 2;
                continue;
            }
        };

        // For renames/copies the next segment is the old path; skip it.
        if status_char == 'R' || status_char == 'C' {
            i += 1;
        }

        // Next segment is the path.
        i += 1;
        let Some(path_seg) = segs.get(i) else {
            break;
        };
        let path_str = std::str::from_utf8(path_seg)
            .unwrap_or("")
            .trim_end_matches('/');
        if path_str.is_empty() {
            i += 1;
            continue;
        }

        let abs = root.join(path_str);
        set_if_higher(&mut map, abs.clone(), status);

        // Include parent directories with highest-priority child status.
        let mut cur = abs.parent();
        while let Some(d) = cur {
            if d == root || !d.starts_with(root) {
                break;
            }
            set_if_higher(&mut map, d.to_path_buf(), status);
            cur = d.parent();
        }

        i += 1;
    }

    map
}

/// Returns the files changed by a single commit, compared with its parent.
/// `--root` also makes the first commit report its added files correctly.
pub fn commit_status(dir: &Path, rev: &str) -> Result<HashMap<PathBuf, GitStatus>, String> {
    let Some(root) = git_toplevel(dir) else {
        return Err("not a git repository".to_string());
    };
    let output = git_cmd()
        .arg("-C")
        .arg(&root)
        .args([
            "diff-tree",
            "--root",
            "--name-status",
            "-r",
            "--format=",
            "-z",
            "--end-of-options",
            rev,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(parse_name_status(&root, &output.stdout))
}

#[cfg(test)]
#[path = "core_test.rs"]
mod tests;
