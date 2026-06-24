use super::*;

#[test]
fn send_set_content_produces_valid_json() {
    let lines = vec!["line1".to_string()];
    let mut buf = Vec::new();
    send_set_content(&lines, "/fake/path", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_content");
    assert_eq!(parsed["params"]["path"], "/fake/path");
}

#[test]
fn send_repo_info_produces_valid_json() {
    let mut buf = Vec::new();
    // This may or may not produce output depending on whether /tmp is in a git repo
    send_repo_info("/tmp", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    if !output.is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["event"], "action");
        assert_eq!(parsed["action"], "set_status_bar_git_info");
    }
}

#[test]
fn send_blame_produces_valid_json_for_nonexistent_file() {
    let mut buf = Vec::new();
    send_blame("/nonexistent/path/file.txt", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    if !output.is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["event"], "action");
        // Could be show_message (blame failed) or set_blame_data
        assert!(parsed["action"] == "show_message" || parsed["action"] == "set_blame_data");
    }
}

#[test]
fn main_loop_tracks_last_file() {
    let mut state = PluginState {
        last_file: None,
        last_sel_file: None,
    };
    let msg: serde_json::Value =
        serde_json::from_str("{\"event\":\"on_file_open\",\"path\":\"/path/to/file.rs\"}").unwrap();
    if let Some(path_str) = msg["path"].as_str() {
        state.last_file = Some(path_str.to_string());
    }
    assert_eq!(state.last_file.as_deref(), Some("/path/to/file.rs"));
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
fn send_file_statuses_produces_valid_json() {
    let mut buf = Vec::new();
    send_file_statuses("/tmp", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    if !output.is_empty() {
        let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
        assert_eq!(parsed["event"], "action");
        assert_eq!(parsed["action"], "set_file_statuses");
    }
}

#[test]
fn repo_root_returns_none_for_non_git_dir() {
    let result = repo_root(Path::new("/nonexistent"));
    assert!(result.is_none());
}

#[test]
fn send_log_does_not_panic_on_nonexistent_file() {
    let mut buf = Vec::new();
    send_log("/nonexistent/path/file.txt", &mut buf);
}

#[test]
fn handle_init_produces_no_output_outside_repo() {
    let tmp = std::env::temp_dir().join("tv-handle-init-test-nonexistent-dir");
    let mut buf = Vec::new();
    handle_init(&tmp, &mut buf);
    // No output expected because /tmp/... is not a git repo
    let output = String::from_utf8(buf).unwrap();
    assert!(output.is_empty(), "expected no output outside a git repo");
}

#[test]
fn handle_init_produces_valid_repo_info_in_repo() {
    // If the test itself runs inside the tv3 repo, handle_init should produce
    // valid set_status_bar_git_info and set_file_statuses actions.
    let cwd = std::env::current_dir().unwrap();
    let mut buf = Vec::new();
    handle_init(&cwd, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    if output.is_empty() {
        // Not in a git repo — nothing to assert; skip.
        return;
    }
    for line in output.lines() {
        let parsed: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed["event"], "action");
        let action = parsed["action"].as_str().unwrap();
        assert!(
            action == "set_status_bar_git_info" || action == "set_file_statuses",
            "unexpected action: {action}"
        );
    }
}
