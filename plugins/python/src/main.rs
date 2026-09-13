//! Bundled Python language provider plugin for mantis.
//!
//! Implements the mantis plugin protocol to provide folding and optional Ruff
//! diagnostics for `.py` and `.pyi` files. On `init`, registers both
//! capabilities. File opens send fold regions from the shared indentation
//! detector; diagnostics requests invoke `ruff check --output-format=json` and
//! return a correlated response. If Ruff is unavailable, diagnostics are empty.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;

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
            "request" if msg["method"].as_str() == Some("diagnostics") => {
                let Some(id) = msg["id"].as_u64() else {
                    continue;
                };
                let Some(path) = msg["params"]["path"].as_str() else {
                    respond_diagnostics(id, Vec::new(), &mut stdout.lock());
                    continue;
                };
                handle_diagnostics(id, path, &mut stdout.lock());
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
            "capabilities": ["fold", "diagnostics"]
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

fn handle_diagnostics(id: u64, path: &str, out: &mut impl Write) {
    let diagnostics = Command::new("ruff")
        .args(["check", "--output-format=json", path])
        .output()
        .ok()
        .filter(|output| !output.stdout.is_empty())
        .and_then(|output| ruff_json_to_diagnostics(&output.stdout).ok())
        .unwrap_or_default();
    respond_diagnostics(id, diagnostics, out);
}

fn respond_diagnostics(id: u64, diagnostics: Vec<serde_json::Value>, out: &mut impl Write) {
    let message = serde_json::json!({
        "event": "response",
        "id": id,
        "result": {"diagnostics": diagnostics}
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&message).unwrap());
    let _ = out.flush();
}

fn ruff_json_to_diagnostics(bytes: &[u8]) -> Result<Vec<serde_json::Value>, serde_json::Error> {
    let messages: Vec<serde_json::Value> = serde_json::from_slice(bytes)?;
    Ok(messages
        .iter()
        .filter_map(|message| {
            let code = message.get("code")?.as_str()?;
            let location = message.get("location")?;
            let line = location.get("row")?.as_u64()?.saturating_sub(1) as usize;
            let column = location.get("column")?.as_u64()?.saturating_sub(1) as usize;
            let end = message.get("end_location");
            let end_line = end
                .and_then(|value| value.get("row"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value.saturating_sub(1) as usize);
            let end_column = end
                .and_then(|value| value.get("column"))
                .and_then(serde_json::Value::as_u64)
                .map(|value| value.saturating_sub(1) as usize);
            let message_text = message.get("message")?.as_str()?;
            let severity = match code.chars().next()? {
                'E' | 'F' => "error",
                'I' => "info",
                _ => "warning",
            };
            Some(serde_json::json!({
                "line": line,
                "column": column,
                "end_line": end_line,
                "end_column": end_column,
                "severity": severity,
                "message": message_text,
                "source": format!("ruff:{code}")
            }))
        })
        .collect())
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
