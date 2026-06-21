//! Bundled git-log plugin for tree-viewer (tv).
//!
//! Implements the tv plugin protocol to display a file's commit history.
//! Tracks the last-opened file via `on_file_open` events (skipping temp files
//! with the `/tmp/tv-git-log` prefix to avoid recursion). On `on_keypress`
//! with key `"H"`, runs `git log --oneline --color=always -- <last_file>` in
//! the repository root, writes the ANSI-coloured output to a temp file, and
//! sends an `open_file` action so the viewer renders the log.
//!
//! Temporary files use the prefix `/tmp/tv-git-log-<pid>-` so the plugin can
//! recognise and skip its own output to avoid recursion.
//!
//! On `shutdown` the event loop exits and the process cleans up its temp files.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

const TMP_PREFIX: &str = "/tmp/tv-git-log";

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let tmp_pattern = format!("{}-{}", TMP_PREFIX, std::process::id());
    let mut tmp_files: Vec<String> = Vec::new();
    let mut last_file: Option<String> = None;

    run_loop(
        stdin.lock(),
        stdout.lock(),
        &tmp_pattern,
        &mut tmp_files,
        &mut last_file,
    );

    for f in &tmp_files {
        let _ = std::fs::remove_file(f);
    }
}

/// Core event loop. Extracted for testability.
pub fn run_loop(
    input: impl BufRead,
    mut output: impl Write,
    tmp_pattern: &str,
    tmp_files: &mut Vec<String>,
    last_file: &mut Option<String>,
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
                    if !path.starts_with(TMP_PREFIX) {
                        *last_file = Some(path.to_string());
                    }
                }
            }
            "on_keypress" => {
                let key = msg["key"].as_str().unwrap_or("");
                if key == "H" {
                    if let Some(ref file) = last_file.clone() {
                        handle_log(file, tmp_pattern, tmp_files, &mut output);
                    }
                }
            }
            "shutdown" => break,
            _ => {}
        }
    }
}

fn handle_log(path: &str, tmp_pattern: &str, tmp_files: &mut Vec<String>, output: &mut impl Write) {
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
    let tmp_path = format!("{}-{:06x}", tmp_pattern, rand_suffix());
    if std::fs::write(&tmp_path, &log_output.stdout).is_err() {
        return;
    }
    tmp_files.push(tmp_path.clone());
    send_open_file(&tmp_path, output);
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

fn send_open_file(path: &str, output: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": "open_file",
        "params": { "path": path }
    });
    let _ = writeln!(
        output,
        "{}",
        serde_json::to_string(&msg).unwrap_or_default()
    );
    let _ = output.flush();
}
