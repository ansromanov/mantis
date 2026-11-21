use super::*;
use std::path::Path;

#[test]
fn ruff_json_maps_locations_severity_and_source() {
    let output = br#"[
        {
            "code":"F821",
            "message":"Undefined name `missing`",
            "location":{"row":3,"column":5},
            "end_location":{"row":3,"column":12}
        },
        {
            "code":"I001",
            "message":"Import block is un-sorted",
            "location":{"row":1,"column":1}
        },
        {"code":"E999","message":"invalid location"}
    ]"#;
    let diagnostics = ruff_json_to_diagnostics(output).unwrap();
    assert_eq!(diagnostics.len(), 2, "entry without a location is skipped");
    assert_eq!(diagnostics[0].line, 2);
    assert_eq!(diagnostics[0].column, 4);
    assert_eq!(diagnostics[0].end_column, Some(11));
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostics[0].source, "ruff:F821");
    assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Info);
}

#[test]
fn other_rule_families_are_warnings() {
    let output = br#"[{"code":"W291","message":"trailing","location":{"row":1,"column":1}}]"#;
    let diagnostics = ruff_json_to_diagnostics(output).unwrap();
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn inverted_ranges_and_empty_messages_are_dropped() {
    let output = br#"[
        {"code":"E1","message":"x","location":{"row":5,"column":1},"end_location":{"row":2,"column":1}},
        {"code":"E2","message":"","location":{"row":1,"column":1}},
        {"code":"E3","message":"ok","location":{"row":1,"column":4},"end_location":{"row":1,"column":2}}
    ]"#;
    assert!(ruff_json_to_diagnostics(output).unwrap().is_empty());
}

#[test]
fn messages_are_sanitized_to_one_line() {
    let output = b"[{\"code\":\"E1\",\"message\":\"a\\n  b\\u001b[31mred\",\"location\":{\"row\":1,\"column\":1}}]";
    let diagnostics = ruff_json_to_diagnostics(output).unwrap();
    assert!(!diagnostics[0].message.contains('\n'));
    assert!(!diagnostics[0].message.contains('\u{1b}'));
}

#[test]
fn invalid_json_is_an_error_not_a_panic() {
    assert!(ruff_json_to_diagnostics(b"not json").is_err());
}

#[test]
fn only_python_files_are_lintable() {
    assert!(is_lintable(Path::new("a.py")));
    assert!(is_lintable(Path::new("a.pyi")));
    assert!(!is_lintable(Path::new("a.rs")));
    assert!(!is_lintable(Path::new("py")));
}
