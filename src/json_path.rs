//! JSON cursor-path mapping for the status bar.
//!
//! This module converts a parsed JSON value into a line-aligned map of JSON
//! paths. The map is built when content is loaded, so rendering only performs
//! a bounded lookup for the active line and never reparses the document.

use serde_json::Value;

/// Builds one JSON path per pretty-printed line, using `None` for punctuation
/// lines that do not identify a value directly.
pub fn build_path_map(value: &Value) -> Vec<Option<String>> {
    let mut map = Vec::new();
    match value {
        Value::Object(_) | Value::Array(_) => {
            map.push(Some(String::new()));
            emit_body(value, String::new(), &mut map);
        }
        _ => map.push(Some(String::new())),
    }
    map
}

fn emit_body(value: &Value, path: String, map: &mut Vec<Option<String>>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{}", key);
                match child {
                    Value::Object(_) | Value::Array(_) => {
                        map.push(Some(child_path.clone()));
                        emit_body(child, child_path, map);
                    }
                    _ => map.push(Some(child_path)),
                }
            }
            map.push(Some(path));
        }
        Value::Array(array) => {
            for (index, child) in array.iter().enumerate() {
                let child_path = format!("{path}[{index}]");
                match child {
                    Value::Object(_) | Value::Array(_) => {
                        map.push(Some(child_path.clone()));
                        emit_body(child, child_path, map);
                    }
                    _ => map.push(Some(child_path)),
                }
            }
            map.push(Some(path));
        }
        _ => {}
    }
}

#[cfg(test)]
#[path = "json_path_test.rs"]
mod tests;
