use std::io::Write;

use super::*;

fn make_vf(content: &[u8]) -> VirtualFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(content).unwrap();
    VirtualFile::open(f.path()).expect("VirtualFile::open failed")
}

#[test]
fn lf_line_endings_stripped() {
    let vf = make_vf(b"hello\nworld\n");
    assert_eq!(vf.line_count(), 2);
    assert_eq!(vf.line_text(0), Some("hello"));
    assert_eq!(vf.line_text(1), Some("world"));
}

#[test]
fn crlf_line_endings_stripped() {
    let vf = make_vf(b"hello\r\nworld\r\n");
    assert_eq!(vf.line_count(), 2);
    assert_eq!(vf.line_text(0), Some("hello"));
    assert_eq!(vf.line_text(1), Some("world"));
}

#[test]
fn no_trailing_empty_line_for_newline_terminated_file() {
    let vf = make_vf(b"a\nb\n");
    assert_eq!(vf.line_count(), 2);
}

#[test]
fn file_without_trailing_newline() {
    let vf = make_vf(b"a\nb");
    assert_eq!(vf.line_count(), 2);
    assert_eq!(vf.line_text(1), Some("b"));
}

#[test]
fn out_of_bounds_returns_none() {
    let vf = make_vf(b"only\n");
    assert_eq!(vf.line_text(1), None);
}

#[test]
fn binary_file_rejected() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"hello\0world").unwrap();
    assert!(VirtualFile::open(f.path()).is_none());
}
