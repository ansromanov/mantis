use super::*;

#[test]
fn test_register_language_provider() {
    let mut buf = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    let extensions = parsed["params"]["extensions"].as_array().unwrap();
    assert!(extensions.contains(&serde_json::json!("sh")));
    assert!(extensions.contains(&serde_json::json!("bash")));
    assert!(extensions.contains(&serde_json::json!("zsh")));
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn test_send_set_fold_regions() {
    let regions = vec![mantis::fold::FoldRegion { start: 1, end: 3 }];
    let mut buf = Vec::new();
    send_set_fold_regions(&regions, "/path/to/file.sh", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], "/path/to/file.sh");
    assert_eq!(parsed["params"]["regions"][0][0], 1);
    assert_eq!(parsed["params"]["regions"][0][1], 3);
}

#[test]
fn test_handle_file_open() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_sh_test.sh");
    std::fs::write(&tmp, "foo() {\n    echo hi\n}\n").unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], path_str);
    assert_eq!(parsed["params"]["regions"][0][0], 0);
    assert_eq!(parsed["params"]["regions"][0][1], 2);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_heredoc() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_sh_test_heredoc.sh");
    // Heredoc contains braces that must not be treated as fold boundaries.
    std::fs::write(
        &tmp,
        "foo() {\n    cat <<EOF\n{ not a fold }\nEOF\n    echo done\n}\n",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "set_fold_regions");
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0][0], 0);
    assert_eq!(regions[0][1], 5);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_comment() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_sh_test_comment.sh");
    // Comment contains braces that must not be treated as fold boundaries.
    std::fs::write(
        &tmp,
        "foo() {\n    # { comment with brace }\n    echo hi\n}\n",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0][0], 0);
    assert_eq!(regions[0][1], 3);

    std::fs::remove_file(&tmp).ok();
}
