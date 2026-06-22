//! Bundled git-log plugin for tree-viewer (tv).
//!
//! Tracks the last-opened file via `on_file_open` events. On `on_keypress`
//! with key `"H"`, runs `git log --oneline --color=always` and sends the
//! output as ANSI-escaped content via `set_content`. Exits cleanly on
//! `shutdown`.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

struct PluginState {
    last_file: Option<String>,
}

fn main() {
    let mut state = PluginState { last_file: None };
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
            "on_file_open" => {
                if let Some(path_str) = msg["path"].as_str() {
                    // Skip temp files created by this or other plugins
                    if !path_str.contains("/tv-git-log-") && !path_str.contains("/tv-git-diff-") {
                        state.last_file = Some(path_str.to_string());
                    }
                }
            }
            "on_keypress" => {
                if let Some(key) = msg["key"].as_str() {
                    if key == "H" {
                        if let Some(ref last) = state.last_file {
                            handle_log(last, &mut io::stdout().lock());
                        }
                    }
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn handle_log(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    let dir = match file_path.parent() {
        Some(d) => d,
        None => return,
    };
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
