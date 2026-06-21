use super::*;

use std::io::Cursor;

#[test]
fn shutdown_terminates_loop() {
    let input = Cursor::new(b"{\"event\":\"shutdown\"}\n");
    let mut output = Vec::new();
    run_loop(input, &mut output);
    // If we reach here, the loop terminated without panic.
}

#[test]
fn init_emits_set_icon_map() {
    let input = Cursor::new(b"{\"event\":\"init\"}\n{\"event\":\"shutdown\"}\n");
    let mut output = Vec::new();
    run_loop(input, &mut output);

    let output_str = String::from_utf8(output).unwrap();
    let msg: serde_json::Value = serde_json::from_str(output_str.trim()).unwrap();

    assert_eq!(msg["event"], "action");
    assert_eq!(msg["action"], "set_icon_map");

    let params = &msg["params"];
    assert!(params["dir_open"].is_string());
    assert!(params["dir_closed"].is_string());
    assert!(params["fallback"].is_string());
    assert!(params["icons"].is_object());
}

#[test]
fn icon_map_contains_expected_extensions() {
    let input = Cursor::new(b"{\"event\":\"init\"}\n{\"event\":\"shutdown\"}\n");
    let mut output = Vec::new();
    run_loop(input, &mut output);

    let output_str = String::from_utf8(output).unwrap();
    let msg: serde_json::Value = serde_json::from_str(output_str.trim()).unwrap();
    let icons = &msg["params"]["icons"];

    for ext in &["rs", "py", "js", "ts", "go", "json", "md", "sh", "lock"] {
        assert!(
            icons[ext].is_string(),
            "Missing icon for extension: {}",
            ext
        );
    }
}

#[test]
fn unknown_events_are_ignored() {
    let input = Cursor::new(
        b"{\"event\":\"on_file_open\",\"path\":\"/foo/bar.rs\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    run_loop(input, &mut output);
    assert!(output.is_empty());
}
