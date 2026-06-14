use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// The current HEAD state of the repository.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GitHead {
    Branch(String),
    #[default]
    Detached,
    Rebase,
    Merge,
}

impl GitHead {
    pub fn display(&self) -> String {
        match self {
            GitHead::Branch(name) => name.clone(),
            GitHead::Detached => "HEAD (detached)".to_string(),
            GitHead::Rebase => "REBASE".to_string(),
            GitHead::Merge => "MERGE".to_string(),
        }
    }
}

/// Rich git repository info: branch/HEAD state, ahead/behind counts, and
/// dirty file counts.
#[derive(Debug, Clone)]
pub struct GitRepoInfo {
    pub head: GitHead,
    pub ahead: usize,
    pub behind: usize,
    /// Total non-ignored changed files.
    pub total_changed: usize,
    /// Files with staged changes.
    pub staged: usize,
    /// Untracked files.
    pub untracked: usize,
}

impl GitRepoInfo {
    pub fn is_dirty(&self) -> bool {
        self.total_changed > 0
    }
}

/// Returns rich git repository info for the directory containing `dir`, or
/// `None` if not in a git repo or git is unavailable. Uses a single
/// `git status --porcelain -b` call.
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

    // Refine HEAD state: check for rebase/merge by inspecting git state files.
    // These checks are unconditional — a merge in progress keeps the branch
    // name in the porcelain header, so we cannot gate on GitHead::Detached.
    let git_dir = root.join(".git");
    if git_dir.join("rebase-merge").exists() || git_dir.join("rebase-apply").exists() {
        info.head = GitHead::Rebase;
    } else if git_dir.join("MERGE_HEAD").exists() {
        info.head = GitHead::Merge;
    }

    Some(info)
}

fn parse_branch_line(line: &str) -> (GitHead, usize, usize) {
    let line = match line.strip_prefix("## ") {
        Some(l) => l,
        None => return (GitHead::default(), 0, 0),
    };

    let head = if line.starts_with("HEAD (no branch)")
        || line.starts_with("Initial commit on ")
        || line.starts_with("No commits yet on ")
    {
        GitHead::Detached
    } else {
        let branch = if let Some(pos) = line.find("...") {
            &line[..pos]
        } else if let Some(pos) = line.find(" [") {
            &line[..pos]
        } else {
            line
        };
        GitHead::Branch(branch.to_string())
    };

    let (ahead, behind) = if let Some(start) = line.find('[') {
        let rest = &line[start + 1..];
        if let Some(end) = rest.find(']') {
            let inner = &rest[..end];
            let mut a = 0usize;
            let mut b = 0usize;
            for part in inner.split(',') {
                let p = part.trim();
                if let Some(n) = p.strip_prefix("ahead ") {
                    a = n.trim().parse().unwrap_or(0);
                } else if let Some(n) = p.strip_prefix("behind ") {
                    b = n.trim().parse().unwrap_or(0);
                }
            }
            (a, b)
        } else {
            (0, 0)
        }
    } else {
        (0, 0)
    };

    (head, ahead, behind)
}

fn parse_repo_info(text: &str) -> GitRepoInfo {
    let mut lines = text.lines();

    // First line is the branch header: "## ..."
    let branch_line = lines.next().unwrap_or("");
    let (head, ahead, behind) = parse_branch_line(branch_line);

    let mut info = GitRepoInfo {
        head,
        ahead,
        behind,
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

        // Skip ignored files.
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

/// Per-line git blame annotation.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct BlameLine {
    pub commit_hash: String,
    pub short_hash: String,
    pub author: String,
    pub date_relative: String,
    pub line_no: u32,
}

#[allow(dead_code)]
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
/// repo, or git is unavailable. Results are cached by (path, mtime) so
/// repeated renders don't re-invoke git.
#[allow(dead_code)]
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
    let mut meta: HashMap<String, (String, String)> = HashMap::new();
    let mut lines = text.lines();

    while let Some(line) = lines.next() {
        // Header: "<hash> <orig_lineno> <final_lineno> [<group_count>]"
        // <group_count> is present only on the first line of a new commit group.
        // Subsequent lines from the same commit have 3 fields and no metadata block.
        let parts: Vec<&str> = line.splitn(4, ' ').collect();
        if parts.len() < 3 {
            continue;
        }
        let hash = parts[0].to_string();
        let line_no: u32 = parts[2].parse().unwrap_or(0);

        let (author, date_relative) = if let Some(m) = meta.get(&hash) {
            m.clone()
        } else {
            let mut author = String::from("Unknown");
            let mut author_time: u64 = 0;

            loop {
                match lines.next() {
                    Some(meta_line) if meta_line.starts_with("author ") => {
                        author = meta_line["author ".len()..].to_string();
                    }
                    Some(meta_line) if meta_line.starts_with("author-time ") => {
                        author_time = meta_line["author-time ".len()..].parse().unwrap_or(0);
                    }
                    Some(meta_line) if meta_line.starts_with("filename ") => break,
                    Some(_) => continue,
                    None => break,
                }
            }

            let date_relative = format_relative_time(author_time);
            meta.insert(hash.clone(), (author.clone(), date_relative.clone()));
            (author, date_relative)
        };

        // Every header (first or repeat) is followed by exactly one tab-prefixed content line.
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
            });
        }
    }

    blames
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn pluralize(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
mod blame_tests {
    use super::parse_blame_porcelain;

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
        // Three consecutive lines from the same commit: first header has group count,
        // subsequent headers have only 3 fields and no metadata block.
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
}
