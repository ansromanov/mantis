//! Binary-file detection and file-encoding/line-ending heuristics.
//!
//! `is_binary_bytes` scans the first `SCAN_LEN` bytes for a NUL byte — a cheap
//! heuristic that keeps the viewer from trying to syntax-highlight binary blobs.
//! `detect_encoding_prefix` classifies the scanned prefix as `"ASCII"` or
//! `"UTF-8 BOM"` without doing a full UTF-8 pass (callers supply `"UTF-8"` once
//! they have confirmed validity via `String::from_utf8` or `VirtualFile::open`).
//! `detect_line_ending` distinguishes LF / CRLF / CR / mixed line endings.
//! All three are consumed by the file loader and virtual-file reader so the
//! info can be shown in the statusbar.

use std::path::Path;

/// Number of leading bytes scanned for a NUL when classifying a file.
const SCAN_LEN: usize = 8192;

/// Heuristic: a NUL byte in the scanned prefix marks the data as binary.
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(SCAN_LEN)].contains(&0u8)
}

/// Classifies the scanned prefix of `bytes` as `"UTF-8 BOM"` or `"ASCII"`,
/// or returns `None` when neither applies (multi-byte UTF-8) or when the prefix
/// contains a NUL byte (binary). The caller is responsible for confirming UTF-8
/// validity via `String::from_utf8` or `VirtualFile::open`, then supplying
/// `"UTF-8"` as the fallback. NUL bytes are rejected here so callers do not need
/// to run `is_binary_bytes` separately before calling this function.
pub fn detect_encoding_prefix(bytes: &[u8]) -> Option<&'static str> {
    let prefix = &bytes[..bytes.len().min(SCAN_LEN)];
    // UTF-8 BOM (\xEF\xBB\xBF)
    if prefix.len() >= 3 && prefix[0] == 0xEF && prefix[1] == 0xBB && prefix[2] == 0xBF {
        return Some("UTF-8 BOM");
    }
    // NUL bytes indicate binary data — NUL is ASCII (0x00) so without this
    // guard the ASCII branch below would misclassify binary files.
    if prefix.contains(&0u8) {
        return None;
    }
    // Pure ASCII (all bytes in 0..128)
    if prefix.iter().all(|&b| b.is_ascii()) {
        return Some("ASCII");
    }
    None
}

/// Detects the predominant line-ending style in the scanned prefix: `"LF"`,
/// `"CRLF"`, `"CR"`, or `"mixed"`. Returns `None` when no line endings are
/// found (single-line or empty file).
pub fn detect_line_ending(bytes: &[u8]) -> Option<&'static str> {
    let scan = &bytes[..bytes.len().min(SCAN_LEN)];
    let mut has_lf = false;
    let mut has_crlf = false;
    let mut has_cr = false;

    let mut i = 0;
    while i < scan.len() {
        if scan[i] == b'\r' {
            if i + 1 < scan.len() && scan[i + 1] == b'\n' {
                has_crlf = true;
                i += 2;
            } else {
                has_cr = true;
                i += 1;
            }
        } else if scan[i] == b'\n' {
            has_lf = true;
            i += 1;
        } else {
            i += 1;
        }
    }

    match (has_lf, has_crlf, has_cr) {
        (true, false, false) => Some("LF"),
        (false, true, false) => Some("CRLF"),
        (false, false, true) => Some("CR"),
        (false, false, false) => None,
        _ => Some("mixed"),
    }
}

/// Formats a byte size in a human-readable format.
fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GiB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Generates a descriptive placeholder content list for a binary file.
pub fn build_binary_placeholder_content(path: Option<&Path>, bytes: &[u8]) -> Vec<String> {
    let type_str = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        "PNG image".to_string()
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "JPEG image".to_string()
    } else if bytes.starts_with(b"GIF89a") || bytes.starts_with(b"GIF87a") {
        "GIF image".to_string()
    } else if bytes.starts_with(b"%PDF-") {
        "PDF document".to_string()
    } else if bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        "ZIP archive".to_string()
    } else if bytes.starts_with(&[0x1F, 0x8B]) {
        "GZIP archive".to_string()
    } else if bytes.starts_with(b"MZ") {
        "executable".to_string()
    } else if bytes.starts_with(&[0x7F, 0x45, 0x4C, 0x46]) {
        "ELF binary".to_string()
    } else if bytes.starts_with(b"SQLite format 3\0") {
        "SQLite database".to_string()
    } else if let Some(ext) = path.and_then(|p| p.extension()).and_then(|e| e.to_str()) {
        match ext.to_lowercase().as_str() {
            "png" => "PNG image".to_string(),
            "jpg" | "jpeg" => "JPEG image".to_string(),
            "gif" => "GIF image".to_string(),
            "pdf" => "PDF document".to_string(),
            "zip" => "ZIP archive".to_string(),
            "gz" | "tgz" => "GZIP archive".to_string(),
            "tar" => "TAR archive".to_string(),
            "exe" => "executable".to_string(),
            "dll" | "so" | "dylib" => "shared library".to_string(),
            "mp3" | "wav" | "ogg" | "flac" => "audio file".to_string(),
            "mp4" | "mkv" | "avi" | "mov" | "webm" => "video file".to_string(),
            "woff" | "woff2" | "ttf" | "otf" => "font file".to_string(),
            other => format!("{} file", other.to_uppercase()),
        }
    } else {
        "unknown type".to_string()
    };

    let size_str = format_size(bytes.len() as u64);
    let mut content = vec![format!("[binary file — {}, {}]", type_str, size_str)];
    if path.is_some() {
        content.push("".into());
        content.push("press o to open with the system default app".into());
    }
    content
}

#[cfg(test)]
#[path = "file_test.rs"]
mod tests;
