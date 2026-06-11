use std::fs;
use std::path::Path;

pub fn is_binary(path: &Path) -> bool {
    match fs::read(path) {
        Ok(bytes) => {
            let check = &bytes[..bytes.len().min(8192)];
            check.contains(&0u8)
        }
        Err(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_file(contents: &[u8]) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("tv_test_{}_{n}.bin", std::process::id()));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn text_file_is_not_binary() {
        let p = temp_file(b"hello\nworld\n");
        assert!(!is_binary(&p));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn file_with_null_byte_is_binary() {
        let p = temp_file(b"abc\0def");
        assert!(is_binary(&p));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn empty_file_is_not_binary() {
        let p = temp_file(b"");
        assert!(!is_binary(&p));
        fs::remove_file(&p).ok();
    }

    #[test]
    fn missing_file_is_binary() {
        let p = std::env::temp_dir().join("tv_test_does_not_exist_xyz");
        assert!(is_binary(&p));
    }

    #[test]
    fn null_byte_past_scan_window_is_ignored() {
        // The scan only inspects the first 8192 bytes.
        let mut data = vec![b'a'; 9000];
        data[8500] = 0;
        let p = temp_file(&data);
        assert!(!is_binary(&p));
        fs::remove_file(&p).ok();
    }
}
