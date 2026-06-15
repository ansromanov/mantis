//! Binary-file detection.
//!
//! A deliberately tiny module: `is_binary_bytes` scans the first `SCAN_LEN`
//! bytes of a buffer for a NUL byte and treats its presence as the signal that
//! the data is binary rather than text. This heuristic is cheap and good enough
//! to keep the viewer from trying to syntax-highlight or render arbitrary binary
//! blobs. It is consumed by the file loader, the virtual-file reader, and
//! content search, all of which fall back to a "binary file" placeholder when
//! this returns `true`. Kept separate so that single rule has one home.

/// Number of leading bytes scanned for a NUL when classifying a file.
const SCAN_LEN: usize = 8192;

/// Heuristic: a NUL byte in the scanned prefix marks the data as binary.
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(SCAN_LEN)].contains(&0u8)
}
