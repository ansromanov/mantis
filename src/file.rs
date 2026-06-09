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
