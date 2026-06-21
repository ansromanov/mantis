//! Bundled git-diff plugin for tree-viewer (tv).
//!
//! On `on_file_open`, if the file is tracked by git, runs
//! `git diff --color=always HEAD` and sends the output as ANSI-escaped
//! content via `set_content`. Exits cleanly on `shutdown`.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

fn main() {
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
                    handle_open(path_str, &mut io::stdout().lock());
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn handle_open(path_str: &str, out: &mut impl Write) {
    let file_path = Path::new(path_str);
    if !file_path.is_file() {
        return;
    }
    let dir = match file_path.parent() {
        Some(d) => d,
        None => return,
    };
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
