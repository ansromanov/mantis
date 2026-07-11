use super::*;

#[test]
fn register_language_provider_produces_valid_json() {
    let mut buf: Vec<u8> = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    assert_eq!(parsed["params"]["extensions"][0], "yaml");
    assert_eq!(parsed["params"]["extensions"][1], "yml");
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn handle_open_skips_non_yaml_files() {
    let mut buf: Vec<u8> = Vec::new();
    handle_open("/dev/null/nonexistent.txt", &mut buf);
    assert!(buf.is_empty(), "no output for non-yaml files");
}

#[test]
fn handle_open_handles_missing_file_gracefully() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_yaml_test_nonexistent.yaml");
    let path_str = tmp.to_str().unwrap().to_string();
    handle_open(&path_str, &mut buf);
    assert!(buf.is_empty(), "no output for non-existent file");
}

#[test]
fn set_fold_regions_simple_nested_key() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_yaml_test_simple.yaml");
    std::fs::write(
        &tmp,
        "\
outer:
  inner: 1
  other: 2
flat: 3
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], path_str);
    assert_eq!(parsed["params"]["regions"][0], serde_json::json!([0, 2]));

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_nested_regions() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_yaml_test_nested.yaml");
    std::fs::write(
        &tmp,
        "\
a:
  b:
    c: 1
  d: 2
e: 3
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert_eq!(parsed["params"]["path"], path_str);
    let regions = parsed["params"]["regions"].as_array().unwrap();
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[0], serde_json::json!([0, 3]));
    assert_eq!(regions[1], serde_json::json!([1, 2]));

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_flat_yaml_no_regions() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_yaml_test_flat.yaml");
    std::fs::write(
        &tmp,
        "\
a: 1
b: 2
c: 3
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["params"]["regions"].as_array().unwrap().len(), 0);

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_yml_extension() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_yaml_test_short_ext.yml");
    std::fs::write(
        &tmp,
        "\
outer:
  inner: 1
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty(), "should produce output for .yml files");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn output_is_single_line_json() {
    let mut buf: Vec<u8> = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let trimmed = output.trim();
    assert!(
        !trimmed.contains('\n'),
        "output should be a single line: {trimmed:?}"
    );
    let _: serde_json::Value = serde_json::from_str(trimmed).unwrap();
}
