//! JSONL display helpers.
//!
//! JSON Lines files contain one JSON value per physical line, which is useful
//! for logs but awkward to read when every object is rendered as raw text.
//! This module provides pure detection and display transformations used by the
//! file loader and content pane: collapsed rows summarize an object on one line,
//! while expanded rows pretty-print one object in place. Invalid lines remain
//! visible so mixed or partially-written logs never disappear.

use std::collections::HashSet;

/// Returns whether `path` is an explicit JSONL/NDJSON path or the content is a
/// JSON object on every non-empty probe line.
pub fn is_jsonl(path: &std::path::Path, lines: &[String]) -> bool {
    let explicit = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(extension.to_ascii_lowercase().as_str(), "jsonl" | "ndjson")
        });
    if explicit {
        return true;
    }
    let non_empty: Vec<&String> = lines
        .iter()
        .filter(|line| !line.trim().is_empty())
        .take(8)
        .collect();
    !non_empty.is_empty()
        && non_empty.iter().all(|line| {
            matches!(
                serde_json::from_str::<serde_json::Value>(line),
                Ok(value) if value.is_object()
            )
        })
}

/// Builds collapsed or expanded display rows and the display-to-source map.
/// Invalid JSON lines are preserved verbatim and map to their source line.
pub fn build_display(source: &[String], expanded: &HashSet<usize>) -> (Vec<String>, Vec<usize>) {
    let mut display = Vec::new();
    let mut map = Vec::new();
    for (source_line, raw) in source.iter().enumerate() {
        let Some(value) = serde_json::from_str::<serde_json::Value>(raw).ok() else {
            display.push(raw.clone());
            map.push(source_line);
            continue;
        };
        if expanded.contains(&source_line) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                for line in pretty.lines() {
                    display.push(line.to_owned());
                    map.push(source_line);
                }
                continue;
            }
        }
        display.push(collapsed(&value));
        map.push(source_line);
    }
    (display, map)
}

fn collapsed(value: &serde_json::Value) -> String {
    let serde_json::Value::Object(object) = value else {
        return value.to_string();
    };
    let fields = object
        .iter()
        .take(4)
        .map(|(key, value)| format!("{key}={}", compact(value)))
        .collect::<Vec<_>>()
        .join(", ");
    if object.len() > 4 {
        format!("{{ {fields}, ... }}")
    } else {
        format!("{{ {fields} }}")
    }
}

fn compact(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => format!("\"{text}\""),
        _ => value.to_string(),
    }
}

#[cfg(test)]
#[path = "jsonl_test.rs"]
mod tests;
