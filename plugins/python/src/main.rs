//! Bundled Python language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide fold capability for `.py`
//! and `.pyi` files. On `init`, registers as a language provider with `fold`
//! capability. On `on_file_open`, reads the file and sends `set_fold_regions`
//! with regions detected by the shared `mantis::fold_detectors::indent_fold`
//! detector.

use std::io::{self, BufRead, Write};
use std::path::Path;

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
                    handle_open(path, &mut stdout.lock());
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
            "extensions": ["py", "pyi"],
            "capabilities": ["fold"]
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn handle_open(path_str: &str, out: &mut impl Write) {
    let path = Path::new(path_str);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "py" && ext != "pyi" {
        return;
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let regions = mantis::fold_detectors::indent_fold(&src);
    let region_pairs: Vec<Vec<usize>> = regions.iter().map(|r| vec![r.start, r.end]).collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_fold_regions",
        "params": {
            "path": path_str,
            "regions": region_pairs
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
