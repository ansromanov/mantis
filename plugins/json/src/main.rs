//! Bundled JSON language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide the `fold` capability
//! for `.json` files. Pretty-printing stays a core rendering concern (see
//! `src/app/loader.rs`); this plugin only adds folding for objects and
//! arrays that core doesn't compute on its own.
//!
//! Core pretty-prints valid JSON for display by default
//! (`serde_json::to_string_pretty`, gated by `prettify_size_limit`), and
//! fold region line numbers are matched against whatever is actually
//! displayed. So `on_file_open` reproduces that same transform — parse then
//! `to_string_pretty` — before running the shared bracket-aware detector,
//! rather than folding the raw (possibly single-line, minified) file bytes.
//! When the JSON fails to parse, core falls back to displaying the raw
//! content verbatim, so this plugin folds the raw content in that case too.
//!
//! One known gap: core's pretty-print is skipped for files over the
//! user-configured `prettify_size_limit` (default 10 MiB), but that limit
//! isn't visible to an out-of-process plugin over the current protocol.
//! This plugin approximates it with the same default; a user who has
//! reconfigured the limit may see misaligned folds on huge files until a
//! protocol change exposes the real value.

use std::io::{self, BufRead, Write};

/// Mirrors `crate::config::types::Config::prettify_size_limit`'s default.
/// See the module doc for why this is an approximation, not a guarantee.
const DEFAULT_PRETTIFY_SIZE_LIMIT: u64 = 10 * 1024 * 1024;

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
            "extensions": ["json"],
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

    let under_size_limit = std::fs::metadata(path)
        .map(|meta| meta.len() <= DEFAULT_PRETTIFY_SIZE_LIMIT)
        .unwrap_or(false);

    let fold_text = if under_size_limit {
        serde_json::from_str::<serde_json::Value>(&content)
            .ok()
            .and_then(|value| serde_json::to_string_pretty(&value).ok())
            .unwrap_or(content)
    } else {
        content
    };

    let regions = mantis::fold_detectors::brace_fold_with_brackets(&fold_text);
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
