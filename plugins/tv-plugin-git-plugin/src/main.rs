//! Bundled comprehensive git plugin for tree-viewer (tv).
//!
//! Handles all git sub-systems that were previously shell scripts:
//! - git status (file statuses for tree coloring)
//! - repo info (branch, HEAD, dirty state in status bar)
//! - working-tree diff (on file open for tracked files)
//! - file log (on H keypress)
//! - file blame (on b keypress)
//!
//! Protocol: receives events on stdin (one JSON object per line) and
//! responds with actions on stdout.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

struct PluginState {
    last_file: Option<String>,
    last_sel_file: Option<String>,
}

fn main() {
    let mut state = PluginState {
        last_file: None,
        last_sel_file: None,
    };
    let stdin = io::stdin();

    for line in stdin.lock().lines() {
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
        match msg["event"].as_str().unwrap_or("") {
            "init" => {}
            "on_file_open" => {
                if let Some(path_str) = msg["path"].as_str() {
                    state.last_file = Some(path_str.to_string());
                    let mut out = io::stdout().lock();
                    send_repo_info(path_str, &mut out);
                    send_file_statuses(path_str, &mut out);
                    send_diff(path_str, &mut out);
                }
            }
            "on_selection_change" => {
                if let Some(path_str) = msg["path"].as_str() {
                    if state.last_sel_file.as_deref() != Some(path_str) {
                        state.last_sel_file = Some(path_str.to_string());
                        send_file_statuses(path_str, &mut io::stdout().lock());
                    }
                }
            }
            "on_keypress" => {
                if let Some(key) = msg["key"].as_str() {
                    match key {
                        "H" => {
                            if let Some(ref last) = state.last_file {
                                send_log(last, &mut io::stdout().lock());
                            }
                        }
                        "b" => {
                            if let Some(ref last) = state.last_file {
                                send_blame(last, &mut io::stdout().lock());
                            }
                        }
                        _ => {}
                    }
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn repo_root(dir: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&output.stdout);
    Some(root.trim().to_string())
}

fn send_repo_info(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    let dir = file_path.parent().unwrap_or(Path::new("."));
    let repo = match repo_root(dir) {
        Some(r) => r,
        None => return,
    };

    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(&repo)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let head = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(&repo)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()
        .ok()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    let state = if Path::new(&repo).join(".git").join("MERGE_HEAD").exists() {
        "merge"
    } else if Path::new(&repo).join(".git").join("rebase-merge").exists()
        || Path::new(&repo).join(".git").join("rebase-apply").exists()
    {
        "rebase"
    } else if dirty {
        "dirty"
    } else {
        "clean"
    };

    let msg = serde_json::json!({
        "event": "action",
        "action": "set_status_bar_git_info",
        "params": {
            "branch": branch,
            "head": head,
            "dirty": dirty,
            "state": state
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn send_file_statuses(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    let dir = file_path.parent().unwrap_or(Path::new("."));
    let repo = match repo_root(dir) {
        Some(r) => r,
        None => return,
    };

    let output = match Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return,
    };
    let status_str = String::from_utf8_lossy(&output);
    if status_str.trim().is_empty() {
        return;
    }

    let mut statuses = HashMap::new();
    for line in status_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let xy = &line[..2.min(line.len())];
        let rest = &line[3.min(line.len())..];
        let path_part = if let Some(idx) = rest.find(" -> ") {
            &rest[idx + 4..]
        } else {
            rest
        };
        let path_part = path_part.trim_end_matches('/');
        if path_part.is_empty() {
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
        let fullpath = format!("{}/{}", repo.trim_end_matches('/'), path_part);
        statuses.insert(fullpath, status);
    }

    if statuses.is_empty() {
        return;
    }

    let msg = serde_json::json!({
        "event": "action",
        "action": "set_file_statuses",
        "params": statuses
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn send_diff(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    if !file_path.is_file() {
        return;
    }
    let dir = file_path.parent().unwrap_or(Path::new("."));
    let repo = match repo_root(dir) {
        Some(r) => r,
        None => return,
    };

    let diff = match Command::new("git")
        .args(["diff", "--color=always", "HEAD", "--", path_str])
        .current_dir(&repo)
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        Ok(_) | Err(_) => return,
    };
    let diff_str = String::from_utf8_lossy(&diff);
    if diff_str.trim().is_empty() {
        return;
    }
    let lines: Vec<String> = diff_str.lines().map(|l| l.to_string()).collect();
    send_set_content(&lines, path_str, out);
}

fn send_log(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    let dir = file_path.parent().unwrap_or(Path::new("."));
    let repo = match repo_root(dir) {
        Some(r) => r,
        None => return,
    };

    let log_output = match Command::new("git")
        .args(["log", "--oneline", "--color=always", "--", path_str])
        .current_dir(&repo)
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        Ok(_) | Err(_) => return,
    };
    let log_str = String::from_utf8_lossy(&log_output);
    if log_str.trim().is_empty() {
        return;
    }
    let lines: Vec<String> = log_str.lines().map(|l| l.to_string()).collect();
    send_set_content(&lines, path_str, out);
}

fn send_blame(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    let dir = file_path.parent().unwrap_or(Path::new("."));
    let repo = match repo_root(dir) {
        Some(r) => r,
        None => return,
    };

    let blame_output = match Command::new("git")
        .args(["blame", "--short", "--", path_str])
        .current_dir(&repo)
        .output()
    {
        Ok(output) if output.status.success() => output.stdout,
        Ok(_) | Err(_) => {
            let msg = serde_json::json!({
                "event": "action",
                "action": "show_message",
                "params": {"message": "blame failed"}
            });
            let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
            let _ = out.flush();
            return;
        }
    };

    let blame_str = String::from_utf8_lossy(&blame_output);
    let mut blame_map: HashMap<usize, String> = HashMap::new();
    let mut total = 0usize;

    for (i, line) in blame_str.lines().enumerate() {
        total = i + 1;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(paren_end) = line.find(") ") {
            let prefix = &line[..=paren_end + 1]; // include ") "
            blame_map.insert(i, prefix.to_string());
        }
    }

    let lines: Vec<String> = (0..total)
        .map(|i| blame_map.get(&i).cloned().unwrap_or_default())
        .collect();

    let msg = serde_json::json!({
        "event": "action",
        "action": "set_blame_data",
        "params": {
            "path": path_str,
            "lines": lines
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn send_set_content(lines: &[String], path: &str, out: &mut impl Write) {
    let json_lines: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::Value::String(l.clone()))
        .collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_content",
        "params": {
            "lines": json_lines,
            "path": path
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
