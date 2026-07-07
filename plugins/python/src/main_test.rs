use super::*;

#[test]
fn register_language_provider_produces_valid_json() {
    let mut buf: Vec<u8> = Vec::new();
    register_language_provider(&mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "register_language_provider");
    assert_eq!(parsed["params"]["extensions"][0], "py");
    assert_eq!(parsed["params"]["extensions"][1], "pyi");
    assert_eq!(parsed["params"]["capabilities"][0], "fold");
}

#[test]
fn handle_open_skips_non_python_files() {
    let mut buf: Vec<u8> = Vec::new();
    handle_open("/dev/null/nonexistent.txt", &mut buf);
    assert!(buf.is_empty(), "no output for non-py files");
}

#[test]
fn handle_open_handles_missing_file_gracefully() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_python_test_nonexistent.py");
    let path_str = tmp.to_str().unwrap().to_string();
    handle_open(&path_str, &mut buf);
    assert!(buf.is_empty(), "no output for non-existent file");
}

#[test]
fn set_fold_regions_simple_def() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_python_test_simple.py");
    std::fs::write(
        &tmp,
        "\
def foo():
    pass
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
    assert_eq!(parsed["params"]["regions"][0], serde_json::json!([0, 1]));

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_triple_quoted_string() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_python_test_triple.py");
    std::fs::write(
        &tmp,
        "\
def docstring_example():
    \"\"\"This is a triple-quoted string.
    It spans multiple lines.
    \"\"\"
    pass

def next_func():
    pass
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
    // def docstring_example: lines 0-5, def next_func: lines 6-7
    assert_eq!(parsed["params"]["regions"][0], serde_json::json!([0, 5]));
    assert_eq!(parsed["params"]["regions"][1], serde_json::json!([6, 7]));

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_else_elif_continuation() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_python_test_continuation.py");
    std::fs::write(
        &tmp,
        "\
if True:
    x = 1
elif other:
    y = 2
else:
    z = 3
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    // One region from `if` through `else` block: lines 0-5
    assert_eq!(parsed["params"]["regions"][0], serde_json::json!([0, 5]));

    std::fs::remove_file(&tmp).ok();
}

#[test]
fn set_fold_regions_pyi_file() {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = std::env::temp_dir();
    tmp.push("mantis_plugin_python_test_stub.pyi");
    std::fs::write(
        &tmp,
        "\
class Processor:
    def run(self) -> None:
        ...
",
    )
    .unwrap();
    let path_str = tmp.to_str().unwrap().to_string();

    handle_open(&path_str, &mut buf);
    let output = String::from_utf8(buf).unwrap();
    assert!(!output.is_empty(), "should produce output for .pyi files");
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_fold_regions");
    assert!(parsed["params"]["regions"].as_array().unwrap().len() >= 2);

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
    // Verify it parses
    let _: serde_json::Value = serde_json::from_str(trimmed).unwrap();
}
