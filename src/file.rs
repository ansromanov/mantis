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
