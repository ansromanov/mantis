//! Display-only secret detection and masking.
//!
//! This module deliberately performs no entropy scoring, persistence, telemetry,
//! or secret reporting. It recognizes common credential-shaped filenames and
//! conservative key/value or token patterns, then replaces only values with a
//! fixed-width ASCII mask so their lengths never reach the terminal.

use std::fs::File;
use std::io::Read;
use std::path::Path;

const MASK: &str = "********";
const CONTENT_PROBE_BYTES: usize = 64 * 1024;

/// Returns whether a path is a credential-shaped file.
pub fn credential_filename(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || lower == "credentials"
        || lower == ".netrc"
        || lower == ".npmrc"
        || lower == ".pypirc"
        || lower == "kubeconfig"
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".tfvars")
        || lower.starts_with("id_rsa")
}

/// Returns whether the beginning of a non-credential file looks secret-shaped.
///
/// This probe lets the loader avoid memory-mapping a file that needs masking.
/// It is deliberately bounded and never returns or logs the sampled content.
pub fn content_probe(path: &Path) -> bool {
    if credential_filename(path) {
        return true;
    }
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut bytes = vec![0; CONTENT_PROBE_BYTES];
    let Ok(count) = file.read(&mut bytes) else {
        return false;
    };
    let Ok(sample) = std::str::from_utf8(&bytes[..count]) else {
        return false;
    };
    sample.lines().any(secret_line)
}

/// Returns whether a line contains a conservative secret-shaped value.
pub fn secret_line(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    if upper.contains("-----BEGIN") && upper.contains("PRIVATE KEY-----") {
        return true;
    }
    let Some((key, value)) = line.split_once('=').or_else(|| line.split_once(':')) else {
        return cloud_token(line.trim()) || jwt_token(line.trim());
    };
    let key = key.trim().to_ascii_uppercase();
    let key_match = [
        "SECRET",
        "TOKEN",
        "PASSWORD",
        "PASSWD",
        "API_KEY",
        "PRIVATE_KEY",
        "CREDENTIAL",
        "AUTH",
    ]
    .iter()
    .any(|word| key.contains(word));
    key_match || cloud_token(value.trim()) || jwt_token(value.trim())
}

fn cloud_token(value: &str) -> bool {
    (value.starts_with("AKIA") && value.len() >= 16)
        || value.starts_with("ghp_")
        || value.starts_with("xoxb-")
        || value.starts_with("sk-")
        || jwt_token(value)
}

fn jwt_token(value: &str) -> bool {
    value.split('.').count() == 3 && value.len() > 30
}

/// Masks the value of a secret-shaped line with a fixed-width placeholder.
pub fn mask_line(line: &str) -> String {
    if !secret_line(line) {
        return line.to_string();
    }
    let separator = [line.find('='), line.find(':')].into_iter().flatten().min();
    if let Some(separator) = separator {
        let value_start = line[separator + 1..]
            .find(|c: char| !c.is_ascii_whitespace())
            .map_or(line.len(), |offset| separator + 1 + offset);
        return format!("{}{MASK}", &line[..value_start]);
    }
    if let Some(index) = line.find("-----BEGIN") {
        return format!("{}{}", &line[..index], MASK);
    }
    MASK.to_string()
}

/// Masks a complete set of display lines when enabled for a credential file.
pub fn mask_lines(path: &Path, lines: &[String], enabled: bool) -> (Vec<String>, bool) {
    let detected = credential_filename(path) || lines.iter().any(|line| secret_line(line));
    if !enabled || !detected {
        return (lines.to_vec(), false);
    }
    let mut in_private_key = false;
    let masked = lines
        .iter()
        .map(|line| {
            let upper = line.to_ascii_uppercase();
            let is_begin = upper.contains("-----BEGIN") && upper.contains("PRIVATE KEY-----");
            let is_end = upper.contains("-----END") && upper.contains("PRIVATE KEY-----");
            let result = if in_private_key || is_begin {
                MASK.to_string()
            } else {
                mask_line(line)
            };
            if is_begin {
                in_private_key = true;
            }
            if is_end {
                in_private_key = false;
            }
            result
        })
        .collect();
    (masked, true)
}

#[cfg(test)]
#[path = "secret_mask_test.rs"]
mod tests;
