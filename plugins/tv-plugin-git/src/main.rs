//! Bundled comprehensive git plugin for tree-viewer (tv).
//!
//! Implements the tv plugin protocol to provide full git integration:
//!   - Repository info (branch, HEAD, dirty state) in the status bar.
//!   - Per-file git statuses for tree colouring.
//!   - Working-tree diff shown on file open (tracked files only).
//!   - Commit history shown on `H` keypress.
//!   - Line-level blame data shown on `b` keypress.
//!
//! Receives JSON events on stdin (one object per line) and responds with JSON
//! actions on stdout. Temp files use `/tmp/tv-git-diff-<pid>-` and
//! `/tmp/tv-git-log-<pid>-` prefixes so the plugin can recognise and skip its
//! own output to avoid recursion.
//!
//! On `shutdown` the event loop exits and all temp files are removed.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

const TMP_DIFF_PREFIX: &str = "/tmp/tv-git-diff";
const TMP_LOG_PREFIX: &str = "/tmp/tv-git-log";

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let pid = std::process::id();
    let diff_pattern = format!("{}-{}", TMP_DIFF_PREFIX, pid);
    let log_pattern = format!("{}-{}", TMP_LOG_PREFIX, pid);
    let mut tmp_files: Vec<String> = Vec::new();
    let mut last_file: Option<String> = None;
    let mut last_sel_file: Option<String> = None;

    run_loop(
        stdin.lock(),
        stdout.lock(),
        &diff_pattern,
        &log_pattern,
        &mut tmp_files,
        &mut last_file,
        &mut last_sel_file,
    );

    for f in &tmp_files {
        let _ = std::fs::remove_file(f);
    }
}

/// Core event loop. Extracted for testability.
#[allow(clippy::too_many_arguments)]
pub fn run_loop(
    input: impl BufRead,
    mut output: impl Write,
    diff_pattern: &str,
    log_pattern: &str,
    tmp_files: &mut Vec<String>,
    last_file: &mut Option<String>,
    last_sel_file: &mut Option<String>,
) {
    for line in input.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    if path.is_empty() {
                        continue;
                    }
                    *last_file = Some(path.to_string());
                    send_repo_info(path, &mut output);
                    send_file_statuses(path, &mut output);
                    send_diff(path, diff_pattern, log_pattern, tmp_files, &mut output);
                }
            }
            "on_selection_change" => {
                if let Some(path) = msg["path"].as_str() {
                    if path.is_empty() {
                        continue;
                    }
                    if last_sel_file.as_deref() == Some(path) {
                        continue;
                    }
                    *last_sel_file = Some(path.to_string());
                    send_file_statuses(path, &mut output);
                }
            }
            "on_keypress" => {
                let key = msg["key"].as_str().unwrap_or("");
                match key {
                    "H" => {
                        if let Some(ref file) = last_file.clone() {
                            send_log(file, log_pattern, tmp_files, &mut output);
                        }
                    }
                    "b" => {
                        if let Some(ref file) = last_file.clone() {
                            if !file.is_empty() {
                                send_blame_data(file, &mut output);
                            }
                        }
                    }
                    _ => {}
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn get_repo_root(file_path: &str) -> Option<String> {
    let dir = Path::new(file_path).parent()?;
    let output = Command::new("git")
        .args(["-C", dir.to_str()?, "rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8(output.stdout).ok()?;
    Some(root.trim().to_string())
}

fn rand_suffix() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.subsec_nanos() ^ (d.as_secs() as u32)) & 0x00ff_ffff)
        .unwrap_or(0)
}

fn emit_action(action: &str, params: serde_json::Value, output: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": action,
        "params": params
    });
    let _ = writeln!(
        output,
        "{}",
        serde_json::to_string(&msg).unwrap_or_default()
    );
    let _ = output.flush();
}

pub fn send_repo_info(file_path: &str, output: &mut impl Write) {
    let repo = match get_repo_root(file_path) {
        Some(r) => r,
        None => return,
    };

    let branch = Command::new("git")
        .args(["-C", &repo, "rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let head = Command::new("git")
        .args(["-C", &repo, "rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    let porcelain = Command::new("git")
        .args(["-C", &repo, "status", "--porcelain"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default();

    let dirty = !porcelain.trim().is_empty();
    let mut state = if dirty { "dirty" } else { "clean" }.to_string();

    // Check for ongoing rebase or merge.
    if Path::new(&format!("{}/.git/rebase-merge", repo)).exists()
        || Path::new(&format!("{}/.git/rebase-apply", repo)).exists()
    {
        state = "rebase".to_string();
    } else if Path::new(&format!("{}/.git/MERGE_HEAD", repo)).exists() {
        state = "merge".to_string();
    }

    emit_action(
        "set_status_bar_git_info",
        serde_json::json!({
            "branch": branch,
            "head": head,
            "dirty": dirty,
            "state": state
        }),
        output,
    );
}

pub fn send_file_statuses(file_path: &str, output: &mut impl Write) {
    let repo = match get_repo_root(file_path) {
        Some(r) => r,
        None => return,
    };

    let porcelain = match Command::new("git")
        .args(["-C", &repo, "status", "--porcelain"])
        .output()
    {
        Ok(o) => match String::from_utf8(o.stdout) {
            Ok(s) => s,
            Err(_) => return,
        },
        Err(_) => return,
    };

    if porcelain.trim().is_empty() {
        return;
    }

    let mut map = serde_json::Map::new();
    for line in porcelain.lines() {
        if line.len() < 4 {
            continue;
        }
        let xy = &line[..2];
        let mut path_str = line[3..].to_string();

        // Handle renames: "R  old -> new" — take the destination.
        if let Some(pos) = path_str.find(" -> ") {
            path_str = path_str[pos + 4..].to_string();
        }
        let path_str = path_str.trim_end_matches('/').trim().to_string();
        if path_str.is_empty() {
            continue;
        }

        let status = match xy {
            "M " | "MM" | " M" => "modified",
            "A " | "AM" | " A" => "added",
            "D " | " D" | "AD" => "deleted",
            "R " | "RM" => "renamed",
            "??" => "untracked",
            "!!" => "ignored",
            "UU" | "AA" | "DD" | "U " | " U" => "conflict",
            _ => continue,
        };

        let full_path = format!("{}/{}", repo, path_str);
        map.insert(full_path, serde_json::Value::String(status.to_string()));
    }

    if map.is_empty() {
        return;
    }

    emit_action("set_file_statuses", serde_json::Value::Object(map), output);
}

pub fn send_diff(
    path: &str,
    diff_pattern: &str,
    log_pattern: &str,
    tmp_files: &mut Vec<String>,
    output: &mut impl Write,
) {
    if path.starts_with(TMP_DIFF_PREFIX) || path.starts_with(TMP_LOG_PREFIX) {
        return;
    }
    if !Path::new(path).is_file() {
        return;
    }
    let repo = match get_repo_root(path) {
        Some(r) => r,
        None => return,
    };
    let diff_output = match Command::new("git")
        .args(["-C", &repo, "diff", "--color=always", "HEAD", "--", path])
        .output()
    {
        Ok(o) => o,
        Err(_) => return,
    };
    if diff_output.stdout.is_empty() {
        return;
    }
    let tmp_path = format!("{}-{:06x}", diff_pattern, rand_suffix());
    if std::fs::write(&tmp_path, &diff_output.stdout).is_err() {
        return;
    }
    tmp_files.push(tmp_path.clone());
    emit_action("open_file", serde_json::json!({ "path": tmp_path }), output);
    let _ = log_pattern; // suppress unused warning
}

pub fn send_log(
    path: &str,
    log_pattern: &str,
    tmp_files: &mut Vec<String>,
    output: &mut impl Write,
) {
    if path.is_empty() {
        return;
    }
    let repo = match get_repo_root(path) {
        Some(r) => r,
        None => return,
    };
    let log_output = match Command::new("git")
        .args([
            "-C",
            &repo,
            "log",
            "--oneline",
            "--color=always",
            "--",
            path,
        ])
        .output()
    {
        Ok(o) => o,
        Err(_) => return,
    };
    if log_output.stdout.is_empty() {
        return;
    }
    let tmp_path = format!("{}-{:06x}", log_pattern, rand_suffix());
    if std::fs::write(&tmp_path, &log_output.stdout).is_err() {
        return;
    }
    tmp_files.push(tmp_path.clone());
    emit_action("open_file", serde_json::json!({ "path": tmp_path }), output);
}

pub fn send_blame_data(path: &str, output: &mut impl Write) {
    let blame_output = match Command::new("git")
        .args(["blame", "--short", "--", path])
        .current_dir(Path::new(path).parent().unwrap_or_else(|| Path::new(".")))
        .output()
    {
        Ok(o) => o,
        Err(_) => return,
    };

    let text = match String::from_utf8(blame_output.stdout) {
        Ok(s) => s,
        Err(_) => return,
    };

    // Parse blame lines: each line format is:
    // `<short-hash> (<Author> <date>)  <line_num>) <content>`
    // We extract the prefix up to and including the first ") " after the paren block.
    let mut prefixes: Vec<String> = Vec::new();
    for bline in text.lines() {
        if bline.is_empty() {
            continue;
        }
        // Find the closing ") " of the paren group.
        if let Some(paren_end) = bline.find(") ") {
            prefixes.push(bline[..paren_end + 2].to_string());
        } else {
            prefixes.push(String::new());
        }
    }

    emit_action(
        "set_blame_data",
        serde_json::json!({ "path": path, "lines": prefixes }),
        output,
    );
}
