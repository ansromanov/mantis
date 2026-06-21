use super::*;
use std::path::Path;

use crate::highlight::Highlighter;

fn make_highlighter() -> Highlighter {
    Highlighter::new("base16-ocean.dark")
}

#[test]
fn test_syntax_only_caps() {
    let provider = SyntaxOnlyProvider::new(make_highlighter());
    let caps = provider.capabilities();
    assert!(caps.highlight);
    assert!(!caps.fold);
    assert!(!caps.hover);
    assert!(!caps.diagnostics);
    assert!(!caps.definition);
}

#[test]
fn test_syntax_fold_caps() {
    let provider = SyntaxFoldProvider::new(make_highlighter());
    let caps = provider.capabilities();
    assert!(caps.highlight);
    assert!(caps.fold);
    assert!(!caps.hover);
}

#[test]
fn test_registry_yaml_default() {
    let registry = LanguageRegistry::new();
    assert!(registry.wants_fold(Path::new("config.yaml")));
    assert!(registry.wants_fold(Path::new("config.yml")));
}

#[test]
fn test_registry_register_extension() {
    let mut registry = LanguageRegistry::new();
    registry.register_fold_extensions(vec!["toml".to_string()]);
    assert!(registry.wants_fold(Path::new("Cargo.toml")));
}

#[test]
fn test_registry_unknown_no_fold() {
    let registry = LanguageRegistry::new();
    assert!(!registry.wants_fold(Path::new("main.rs")));
}

#[test]
fn test_syntax_fold_provider_fold_regions() {
    let provider = SyntaxFoldProvider::new(make_highlighter());
    let lines: Vec<String> = vec![
        "root:".to_string(),
        "  child:".to_string(),
        "    grandchild: value".to_string(),
        "other: value".to_string(),
    ];
    let regions = provider.fold_regions(&lines);
    assert!(
        !regions.is_empty(),
        "expected non-empty fold regions for YAML-like indented content"
    );
}
