//! Bundled Rust language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide language services for `.rs`
//! files. It registers the `fold` and `symbols` capabilities, then runs the shared
//! Rust detectors on each open file and returns fold regions and outline entries.

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
            "extensions": ["rs"],
            "capabilities": ["fold", "symbols"],
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
    let symbols = mantis::fold_detectors::rust_symbols(&content);
    send_set_symbols(&symbols, path, out);
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

fn send_set_symbols(symbols: &[mantis::plugin::types::Symbol], path: &str, out: &mut impl Write) {
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_symbols",
        "params": {
            "path": path,
            "symbols": symbols
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
