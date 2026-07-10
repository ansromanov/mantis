use super::*;

#[test]
fn test_register_language_provider() {
    let mut buf = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    assert_eq!(parsed["params"]["extensions"][0], "json");
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn test_send_set_fold_regions() {
    let regions = vec![mantis::fold::FoldRegion { start: 1, end: 3 }];
    let mut buf = Vec::new();
    send_set_fold_regions(&regions, "/path/to/file.json", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], "/path/to/file.json");
    assert_eq!(parsed["params"]["regions"][0][0], 1);
    assert_eq!(parsed["params"]["regions"][0][1], 3);
}

/// Fold regions must be computed against the *pretty-printed* line numbers
/// the user sees — core reformats valid JSON for display by default — not
/// against the raw single-line source.
#[test]
fn test_handle_file_open_folds_pretty_printed_minified_json() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_json_test_minified.json");
    std::fs::write(&tmp, r#"{"a":1,"b":[1,2,3]}"#).unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], path_str);

    // Same text core would compute: parse then to_string_pretty.
    let value: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":[1,2,3]}"#).unwrap();
    let pretty = serde_json::to_string_pretty(&value).unwrap();
    let expected = mantis::fold_detectors::brace_fold_with_brackets(&pretty);
    assert_eq!(expected.len(), 2);

    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), expected.len());
    for (got, want) in regions.iter().zip(expected.iter()) {
        assert_eq!(got[0], want.start);
        assert_eq!(got[1], want.end);
    }

    std::fs::remove_file(&tmp).ok();
}

/// Invalid JSON can't be pretty-printed — core falls back to displaying the
/// raw file content in that case, so the plugin must fold the raw content
/// too, not silently emit nothing.
#[test]
fn test_handle_file_open_falls_back_to_raw_on_invalid_json() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_json_test_invalid.json");
    std::fs::write(&tmp, "{\n    \"a\": 1,\n    not valid json\n}\n").unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["action"], "set_fold_regions");
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0][0], 0);
    assert_eq!(regions[0][1], 3);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn test_handle_file_open_single_line_object_no_regions() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_json_test_scalar.json");
    std::fs::write(&tmp, "42").unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    let mut buf = Vec::new();
    handle_file_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert!(regions.is_empty());

    std::fs::remove_file(&tmp).ok();
}
