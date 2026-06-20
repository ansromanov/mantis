//! Binary-file detection and file-encoding/line-ending heuristics.
//!
//! `is_binary_bytes` scans the first `SCAN_LEN` bytes for a NUL byte — a cheap
//! heuristic that keeps the viewer from trying to syntax-highlight binary blobs.
//! `detect_encoding` classifies raw bytes as ASCII / UTF-8 / UTF-8 BOM, and
//! `detect_line_ending` distinguishes LF / CRLF / CR / mixed line endings.
//! All three are consumed by the file loader and virtual-file reader so the
//! info can be shown in the statusbar.

/// Number of leading bytes scanned for a NUL when classifying a file.
const SCAN_LEN: usize = 8192;

/// Heuristic: a NUL byte in the scanned prefix marks the data as binary.
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(SCAN_LEN)].contains(&0u8)
}

/// Classifies `bytes` as one of `"ASCII"`, `"UTF-8"`, `"UTF-8 BOM"`, or
/// returns `None` when the encoding cannot be determined (binary or invalid
/// UTF-8 without a BOM).
pub fn detect_encoding(bytes: &[u8]) -> Option<&'static str> {
    let prefix = &bytes[..bytes.len().min(SCAN_LEN)];
    // UTF-8 BOM (\xEF\xBB\xBF)
    if prefix.len() >= 3 && prefix[0] == 0xEF && prefix[1] == 0xBB && prefix[2] == 0xBF {
        return Some("UTF-8 BOM");
    }
    // Pure ASCII (all bytes in 0..128)
    if prefix.iter().all(|&b| b.is_ascii()) {
        return Some("ASCII");
    }
    // Valid UTF-8 (must hold for the whole buffer, not just the prefix, but
    // the prefix check is a good heuristic for files the viewer opens).
    if std::str::from_utf8(bytes).is_ok() {
        return Some("UTF-8");
    }
    None
}

/// Detects the predominant line-ending style: `"LF"`, `"CRLF"`, `"CR"`, or
/// `"mixed"`. Returns `None` when no line endings are found (single-line or
/// empty file).
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
