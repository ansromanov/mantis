//! Python diagnostics from the external `ruff` linter.
//!
//! Replaces the former bundled Python plugin. [`run_ruff`] shells out to
//! `ruff check --output-format=json` for one file and [`ruff_json_to_diagnostics`]
//! maps its output to [`crate::code_intel::Diagnostic`] records. When `ruff` is
//! not installed (or fails) the result is simply empty, so diagnostics are an
//! optional extra that never gets in the way of viewing a file.
//!
//! Public items:
//! - [`is_lintable`]: whether a path is a Python file.
//! - [`run_ruff`]: runs the linter for one file (blocking; call off the UI thread).
//! - [`ruff_json_to_diagnostics`]: parses ruff's JSON output.

use std::path::Path;
use std::process::Command;

use crate::code_intel::{Diagnostic, DiagnosticSeverity};

/// Upper bound on diagnostics kept per file, so a pathological file cannot flood the UI.
const MAX_DIAGNOSTICS: usize = 10_000;

/// Whether `path` is a Python source file that ruff can lint.
pub fn is_lintable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == "py" || e == "pyi")
}

/// Runs `ruff check` on `path`. Returns an empty list if ruff is missing, fails,
/// or prints nothing parsable.
pub fn run_ruff(path: &Path) -> Vec<Diagnostic> {
    Command::new("ruff")
        .args(["check", "--output-format=json"])
        .arg(path)
        .output()
        .ok()
        .filter(|output| !output.stdout.is_empty())
        .and_then(|output| ruff_json_to_diagnostics(&output.stdout).ok())
        .unwrap_or_default()
}

/// Maps ruff's JSON array to diagnostics. Entries missing a code, message or
/// location are skipped. `E`/`F` rules are errors, `I` is info, the rest warnings.
pub fn ruff_json_to_diagnostics(bytes: &[u8]) -> Result<Vec<Diagnostic>, serde_json::Error> {
    let messages: Vec<serde_json::Value> = serde_json::from_slice(bytes)?;
    Ok(messages
        .iter()
        .take(MAX_DIAGNOSTICS)
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
            let text = message.get("message")?.as_str()?;
            if text.is_empty()
                || end_line.is_some_and(|end| end < line)
                || (end_line == Some(line) && end_column.is_some_and(|end| end < column))
            {
                return None;
            }
            let severity = match code.chars().next()? {
                'E' | 'F' => DiagnosticSeverity::Error,
                'I' => DiagnosticSeverity::Info,
                _ => DiagnosticSeverity::Warning,
            };
            Some(Diagnostic {
                line,
                column,
                end_line,
                end_column,
                severity,
                message: single_line(text),
                source: format!("ruff:{code}"),
            })
        })
        .collect())
}

/// Strips terminal control sequences and collapses whitespace so a message
/// always renders as one clean line.
fn single_line(text: &str) -> String {
    crate::ansi::sanitize_terminal_text(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "lint_test.rs"]
mod tests;
