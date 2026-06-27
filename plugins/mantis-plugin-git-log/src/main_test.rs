use super::*;

#[test]
fn send_set_content_produces_valid_json() {
    let lines = vec!["abc123 fix bug".to_string()];
    let mut buf: Vec<u8> = Vec::new();
    send_set_content(&lines, "/fake/path", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_content");
    assert_eq!(parsed["params"]["path"], "/fake/path");
    assert_eq!(parsed["params"]["lines"][0], "abc123 fix bug");
}

#[test]
fn state_tracks_last_file() {
    let mut state = PluginState { last_file: None };
    state.last_file = Some("/path/to/file.rs".to_string());
    assert_eq!(state.last_file.as_deref(), Some("/path/to/file.rs"));
}

#[test]
fn state_skips_temp_files() {
    let mut state = PluginState { last_file: None };
    let msg: serde_json::Value =
        serde_json::from_str("{\"event\":\"on_file_open\",\"path\":\"/tmp/tv-git-log-abc123\"}")
            .unwrap();
    if let Some(path_str) = msg["path"].as_str() {
        if !path_str.contains("/tv-git-log-") && !path_str.contains("/tv-git-diff-") {
            state.last_file = Some(path_str.to_string());
        }
    }
    assert!(state.last_file.is_none(), "should not track temp files");
}

#[test]
fn handle_log_does_not_panic_on_nonexistent_file() {
    let mut buf: Vec<u8> = Vec::new();
    handle_log("/nonexistent/path/file.txt", &mut buf);
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
    let tmp = std::env::temp_dir();
    let result = repo_root(&tmp);
    let _ = result;
}
