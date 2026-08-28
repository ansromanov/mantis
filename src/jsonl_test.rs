use super::*;

use std::collections::HashSet;
use std::path::Path;

#[test]
fn detects_explicit_jsonl_paths() {
    assert!(is_jsonl(Path::new("events.jsonl"), &["garbage".into()]));
    assert!(is_jsonl(Path::new("events.ndjson"), &["garbage".into()]));
}

#[test]
fn detects_object_lines_without_jsonl_extension() {
    assert!(is_jsonl(
        Path::new("events.log"),
        &[r#"{"level":"info"}"#.into(), r#"{"level":"error"}"#.into()]
    ));
    assert!(!is_jsonl(Path::new("events.log"), &["plain text".into()]));
}

#[test]
fn does_not_classify_single_line_json_as_jsonl() {
    assert!(!is_jsonl(
        Path::new("config.json"),
        &[r#"{"key":true}"#.into()]
    ));
}

#[test]
fn collapsed_rows_preserve_invalid_lines() {
    let source = vec![
        r#"{"level":"info","message":"ready"}"#.into(),
        "partial".into(),
    ];
    let (display, map) = build_display(&source, &HashSet::new());
    assert_eq!(display[0], r#"{ level="info", message="ready" }"#);
    assert_eq!(display[1], "partial");
    assert_eq!(map, vec![0, 1]);
}

#[test]
fn expanded_row_maps_each_pretty_line_to_source() {
    let source = vec![r#"{"level":"info","nested":{"ok":true}}"#.into()];
    let mut expanded = HashSet::new();
    expanded.insert(0);
    let (display, map) = build_display(&source, &expanded);
    assert!(display.len() > 1);
    assert!(display.iter().any(|line| line.contains("nested")));
    assert!(map.iter().all(|&line| line == 0));
}
