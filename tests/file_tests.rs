use mantis::file::is_binary_bytes;

#[test]
fn text_bytes_are_not_binary() {
    assert!(!is_binary_bytes(b"hello\nworld\n"));
}

#[test]
fn null_byte_is_binary() {
    assert!(is_binary_bytes(b"abc\0def"));
}

#[test]
fn empty_bytes_are_not_binary() {
    assert!(!is_binary_bytes(b""));
}

#[test]
fn null_byte_past_scan_window_is_ignored() {
    let mut data = vec![b'a'; 9000];
    data[8500] = 0;
    assert!(!is_binary_bytes(&data));
}
