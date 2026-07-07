//! Bundled Go language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide language services for `.go`
//! files. Today, it registers the `fold` capability and responds to `on_file_open`
//! events by running the shared `brace_fold` detector and returning the fold regions.

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

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

        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "init" => {
                register_language_provider(&mut stdout.lock());
            }
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    handle_file_open(path, &mut stdout.lock());
                }
            }
            "on_quit" | "shutdown" => break,
            _ => {}
        }
    }
}

fn register_language_provider(out: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": "register_language_provider",
        "params": {
            "extensions": ["go"],
            "capabilities": ["fold"],
            "priority": 0
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn handle_file_open(path: &str, out: &mut impl Write) {
    let content = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let regions = mantis::fold_detectors::brace_fold(&content);
    send_set_fold_regions(&regions, path, out);
}

fn send_set_fold_regions(regions: &[mantis::fold::FoldRegion], path: &str, out: &mut impl Write) {
    let json_regions: Vec<serde_json::Value> = regions
        .iter()
        .map(|r| serde_json::json!([r.start, r.end]))
        .collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_fold_regions",
        "params": {
            "path": path,
            "regions": json_regions
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
