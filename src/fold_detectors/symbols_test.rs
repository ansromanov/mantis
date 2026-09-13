use super::*;

#[test]
fn rust_detector_finds_types_impls_and_methods_with_ranges() {
    let source = "struct Service {\n    value: usize,\n}\nimpl Service {\n    fn run(&self) {\n        self.value;\n    }\n}\n";
    let symbols = rust_symbols(source);
    assert_eq!(symbols.len(), 3);
    assert_eq!(
        (symbols[0].name.as_str(), symbols[0].kind.as_str()),
        ("Service", "struct")
    );
    assert_eq!(symbols[0].end_line, Some(2));
    assert_eq!(
        (symbols[1].name.as_str(), symbols[1].kind.as_str()),
        ("Service", "impl")
    );
    assert_eq!(
        (symbols[2].name.as_str(), symbols[2].kind.as_str()),
        ("run", "method")
    );
    assert_eq!(symbols[2].parent.as_deref(), Some("Service"));
    assert_eq!(symbols[2].end_line, Some(6));
}

#[test]
fn rust_detector_finds_free_functions_and_modules() {
    let symbols = rust_symbols("mod api {\n    fn call() {}\n}\nfn top() {}\n");
    assert_eq!(
        symbols
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        ["api", "call", "top"]
    );
    assert_eq!(symbols[1].parent.as_deref(), Some("api"));
    assert_eq!(symbols[2].kind, "function");
}
