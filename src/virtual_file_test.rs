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

#[test]
fn external_truncation_does_not_sigbus_owned() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"line1\nline2\nline3\n").unwrap();
    f.flush().unwrap();

    let vf = VirtualFile::open(f.path()).expect("VirtualFile::open failed");
    assert_eq!(vf.line_count(), 3);
    assert_eq!(vf.line_text(0), Some("line1"));
    assert_eq!(vf.line_text(1), Some("line2"));
    assert_eq!(vf.line_text(2), Some("line3"));

    // Truncate the backing file (simulates : >file, editor save-with-truncate,
    // log rotation, or git checkout of a smaller revision).
    f.as_file_mut().set_len(0).unwrap();
    f.flush().unwrap();

    // The file is ≤ MMAP_THRESHOLD so VirtualFile owns the data in memory.
    // line_text must still return the original content; a SIGBUS would mean
    // we're reading from a stale mmap.
    assert_eq!(vf.line_count(), 3);
    assert_eq!(vf.line_text(0), Some("line1"));
    assert_eq!(vf.line_text(1), Some("line2"));
    assert_eq!(vf.line_text(2), Some("line3"));
}

#[test]
fn update_growth_appends_new_lines_owned() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"line1\nline2\n").unwrap();
    f.flush().unwrap();

    let mut vf = VirtualFile::open(f.path()).unwrap();
    assert_eq!(vf.line_count(), 2);
    assert_eq!(vf.line_text(1), Some("line2"));

    // Append some content
    f.write_all(b"line3\n").unwrap();
    f.flush().unwrap();

    let res = vf.update_growth(f.path());
    assert_eq!(res, Some(true));
    assert_eq!(vf.line_count(), 3);
    assert_eq!(vf.line_text(2), Some("line3"));
}

#[test]
fn update_growth_truncation_reopens() {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(b"line1\nline2\n").unwrap();
    f.flush().unwrap();

    let mut vf = VirtualFile::open(f.path()).unwrap();
    assert_eq!(vf.line_count(), 2);

    // Truncate it
    f.as_file_mut().set_len(0).unwrap();
    f.flush().unwrap();

    let res = vf.update_growth(f.path());
    assert_eq!(res, Some(false));
}
