use super::*;

#[test]
fn test_register_language_provider() {
    let mut buf = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    assert_eq!(parsed["params"]["extensions"][0], "rs");
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn test_send_set_fold_regions() {
    let regions = vec![mantis::fold::FoldRegion { start: 1, end: 3 }];
    let mut buf = Vec::new();
    send_set_fold_regions(&regions, "/path/to/file.rs", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], "/path/to/file.rs");
    assert_eq!(parsed["params"]["regions"][0][0], 1);
    assert_eq!(parsed["params"]["regions"][0][1], 3);
}

#[test]
fn test_handle_file_open() {
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_rust_test.rs");
    std::fs::write(&tmp, "fn foo() {\n    let x = 1;\n}").unwrap();
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
