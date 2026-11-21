use super::*;

fn symbol(name: &str, line: usize, end_line: usize, parent: Option<&str>) -> Symbol {
    Symbol {
        name: name.to_string(),
        kind: "function".to_string(),
        line,
        end_line: Some(end_line),
        parent: parent.map(str::to_string),
    }
}

#[test]
fn enclosing_symbol_selects_the_narrowest_range_and_includes_boundaries() {
    let symbols = [
        symbol("outer", 2, 20, None),
        symbol("inner", 5, 8, Some("outer")),
    ];
    assert_eq!(
        enclosing_symbol(&symbols, 5).map(|item| item.name.as_str()),
        Some("inner")
    );
    assert_eq!(
        enclosing_symbol(&symbols, 8).map(|item| item.name.as_str()),
        Some("inner")
    );
    assert_eq!(
        enclosing_symbol(&symbols, 9).map(|item| item.name.as_str()),
        Some("outer")
    );
}

#[test]
fn symbol_without_end_line_matches_only_its_declaration() {
    let symbol = Symbol {
        name: "field".into(),
        kind: "key".into(),
        line: 3,
        end_line: None,
        parent: None,
    };
    assert!(enclosing_symbol(std::slice::from_ref(&symbol), 3).is_some());
    assert!(enclosing_symbol(&[symbol], 4).is_none());
}

#[test]
fn diagnostic_deserializes_optional_ranges_and_lowercase_severity() {
    let diagnostic: Diagnostic = serde_json::from_value(serde_json::json!({
        "line":4,
        "column":2,
        "severity":"warning",
        "message":"unused value",
        "source":"ruff:F841"
    }))
    .unwrap();
    assert_eq!(diagnostic.line, 4);
    assert_eq!(diagnostic.end_line, None);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert!(serde_json::from_value::<Diagnostic>(serde_json::json!({
        "line":4,
        "column":2,
        "severity":"critical",
        "message":"bad severity",
        "source":"test"
    }))
    .is_err());
}
