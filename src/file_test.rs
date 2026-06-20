use super::*;

// --- is_binary_bytes ---

#[test]
fn empty_bytes_not_binary() {
    assert!(!is_binary_bytes(b""));
}

#[test]
fn nul_byte_is_binary() {
    assert!(is_binary_bytes(b"hello\0world"));
}

#[test]
fn text_bytes_are_not_binary() {
    assert!(!is_binary_bytes(b"fn main() {}\n"));
}

#[test]
fn null_byte_past_scan_window_is_ignored() {
    let mut data = vec![b'a'; 8192];
    data.push(0u8);
    assert!(!is_binary_bytes(&data));
}

// --- detect_encoding_prefix ---

#[test]
fn prefix_detects_utf8_bom() {
    assert_eq!(
        detect_encoding_prefix(b"\xEF\xBB\xBFhello"),
        Some("UTF-8 BOM")
    );
}

#[test]
fn prefix_detects_ascii() {
    assert_eq!(detect_encoding_prefix(b"hello world\n"), Some("ASCII"));
}

#[test]
fn prefix_returns_none_for_multibyte_utf8() {
    // Caller supplies "UTF-8" after String::from_utf8 confirms validity.
    assert_eq!(detect_encoding_prefix("héllo\n".as_bytes()), None);
}

#[test]
fn prefix_returns_none_for_nul_byte() {
    assert_eq!(detect_encoding_prefix(b"data\x00binary"), None);
}

#[test]
fn prefix_empty_input_is_ascii() {
    assert_eq!(detect_encoding_prefix(b""), Some("ASCII"));
}

// --- detect_line_ending ---

#[test]
fn line_ending_empty() {
    assert_eq!(detect_line_ending(b""), None);
}

#[test]
fn line_ending_no_newlines() {
    assert_eq!(detect_line_ending(b"single line"), None);
}

#[test]
fn line_ending_lf() {
    assert_eq!(detect_line_ending(b"a\nb\nc\n"), Some("LF"));
}

#[test]
fn line_ending_crlf() {
    assert_eq!(detect_line_ending(b"a\r\nb\r\nc\r\n"), Some("CRLF"));
}

#[test]
fn line_ending_cr_only() {
    assert_eq!(detect_line_ending(b"a\rb\rc\r"), Some("CR"));
}

#[test]
fn line_ending_mixed_lf_and_crlf() {
    assert_eq!(detect_line_ending(b"a\nb\r\nc\r"), Some("mixed"));
}
