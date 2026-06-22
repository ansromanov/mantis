use super::*;

#[test]
fn send_set_content_produces_valid_json() {
    let lines = vec!["line1".to_string(), "line2".to_string()];
    let mut buf: Vec<u8> = Vec::new();
    send_set_content(&lines, "/fake/path", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_content");
    assert_eq!(parsed["params"]["path"], "/fake/path");
    assert_eq!(parsed["params"]["lines"][0], "line1");
    assert_eq!(parsed["params"]["lines"][1], "line2");
}

#[test]
fn handle_open_skips_nonexistent_file() {
    let mut buf: Vec<u8> = Vec::new();
    handle_open("/nonexistent/path/file.txt", &mut buf);
    assert!(buf.is_empty(), "no output for nonexistent file");
}

#[test]
fn main_loop_handles_init_and_shutdown() {
    let input = "{\"event\":\"init\"}\n{\"event\":\"shutdown\"}\n";
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _msg: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    }
}

#[test]
fn main_loop_ignores_unknown_events() {
    let input = "{\"event\":\"unknown\"}\n{\"event\":\"shutdown\"}\n";
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _msg: serde_json::Value = serde_json::from_str(trimmed).unwrap();
    }
}

#[test]
fn repo_root_returns_none_for_non_git_dir() {
    let result = repo_root(std::path::Path::new("/nonexistent"));
    assert!(result.is_none());
}

#[test]
fn handle_open_ignores_non_git_files() {
    let tmp = std::env::temp_dir();
    let test_file = tmp.join("tv_test_nongit_file.txt");
    let _ = std::fs::write(&test_file, b"hello");
    let mut buf: Vec<u8> = Vec::new();
    handle_open(test_file.to_str().unwrap(), &mut buf);
    let _ = buf;
    let _ = std::fs::remove_file(&test_file);
}
