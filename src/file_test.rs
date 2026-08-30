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

#[test]
fn format_size_works() {
    assert_eq!(format_size(100), "100 B");
    assert_eq!(format_size(1024), "1.0 KiB");
    assert_eq!(format_size(1024 * 1024), "1.0 MiB");
    assert_eq!(format_size(1572864), "1.5 MiB");
}

#[test]
fn build_binary_placeholder_content_works() {
    // Bare magic bytes with no IHDR chunk: recognized as PNG, but too short
    // for the header parser to extract real dimensions from.
    let png_magic = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    let path = Path::new("test.png");
    let content = build_binary_placeholder_content(Some(path), png_magic);
    assert_eq!(content[0], "[image file — PNG, 8 B]");
    assert_eq!(content[2], "press o to open with the system default app");

    let unknown_magic = &[0u8; 12];
    let no_ext_path = Path::new("test");
    let content2 = build_binary_placeholder_content(Some(no_ext_path), unknown_magic);
    assert_eq!(content2[0], "[binary file — unknown type, 12 B]");

    let content_no_path = build_binary_placeholder_content(None, png_magic);
    assert_eq!(content_no_path.len(), 1);
    assert_eq!(content_no_path[0], "[image file — PNG, 8 B]");
}

/// Encodes a real `width`x`height` image via the `image` crate's own PNG
/// encoder, so the bytes carry a real IHDR chunk the dimension reader can
/// parse — avoids hand-computing PNG chunk CRCs in the test.
fn encode_test_png(width: u32, height: u32) -> Vec<u8> {
    let img = image::RgbImage::new(width, height);
    let mut buf = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .expect("encode test PNG");
    buf.into_inner()
}

#[test]
fn build_binary_placeholder_content_reports_real_image_dimensions() {
    let bytes = encode_test_png(64, 32);
    let path = Path::new("photo.png");
    let content = build_binary_placeholder_content(Some(path), &bytes);
    assert!(
        content[0].starts_with("[image file — PNG, 64x32, "),
        "expected dimensions in {:?}",
        content[0]
    );
}

#[test]
fn image_format_label_falls_back_to_extension_for_other_raster_formats() {
    // BMP/WebP aren't covered by the earlier magic-byte tests; exercise their
    // extension fallback and confirm they get the new "image file" prefix
    // rather than falling through to the generic "BMP file"/"WEBP file" the
    // old extension-only match used to produce.
    let content = build_binary_placeholder_content(Some(Path::new("icon.bmp")), &[0u8; 4]);
    assert_eq!(content[0], "[image file — BMP, 4 B]");

    let content = build_binary_placeholder_content(Some(Path::new("icon.webp")), &[0u8; 4]);
    assert_eq!(content[0], "[image file — WebP, 4 B]");
}

#[test]
fn is_markdown_path_works() {
    assert!(is_markdown_path(Path::new("test.md")));
    assert!(is_markdown_path(Path::new("test.markdown")));
    assert!(!is_markdown_path(Path::new("test.txt")));
    assert!(!is_markdown_path(Path::new("test")));
}

#[test]
fn is_log_file_works() {
    assert!(is_log_file(Path::new("test.log"), &[]));
    assert!(is_log_file(
        Path::new("test.txt"),
        &["[INFO] hello", "[ERROR] world"]
    ));
    assert!(is_log_file(
        Path::new("test.txt"),
        &["2026-07-19 12:00:00 message"]
    ));
    assert!(!is_log_file(
        Path::new("test.txt"),
        &["not a log line", "plain text line"]
    ));
}
