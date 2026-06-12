/// Number of leading bytes scanned for a NUL when classifying a file.
const SCAN_LEN: usize = 8192;

/// Heuristic: a NUL byte in the scanned prefix marks the data as binary.
pub fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(SCAN_LEN)].contains(&0u8)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // The scan only inspects the first SCAN_LEN bytes.
        let mut data = vec![b'a'; 9000];
        data[8500] = 0;
        assert!(!is_binary_bytes(&data));
    }
}
