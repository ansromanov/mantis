use super::*;
use std::path::Path;

#[test]
fn new_with_valid_theme() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    assert_eq!(h.theme, "base16-ocean.dark");
}

#[test]
fn new_falls_back_for_unknown_theme() {
    let h = Highlighter::with_extra_syntaxes("nonexistent-theme-name", &[]);
    assert_eq!(h.theme, "base16-ocean.dark");
}

#[test]
fn highlight_returns_one_vec_per_line() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["hello".to_string(), "world".to_string()];
    let result = h.highlight(Path::new("f.txt"), &lines);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0][0].1, "hello");
    assert_eq!(result[1][0].1, "world");
}

#[test]
fn highlight_plain_text_no_extra_styling() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight(Path::new("f.txt"), &[":)".to_string()]);
    assert_eq!(result[0][0].1, ":)");
    assert_eq!(result[0][0].0.add_modifier, Modifier::empty());
}

#[test]
fn highlight_rust_code_colors_keywords() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight(Path::new("main.rs"), &["fn main() {".to_string()]);
    assert!(
        result[0].len() > 1,
        "expected multiple spans for Rust code, got {}",
        result[0].len()
    );
    let has_fg = result[0].iter().any(|(s, _)| s.fg.is_some());
    assert!(has_fg, "Rust code should have some colored spans");
}

#[test]
fn highlight_state_tracks_across_lines() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["/// doc comment".to_string(), "fn main() {}".to_string()];
    let result = h.highlight(Path::new("main.rs"), &lines);
    assert_eq!(result.len(), 2);
    assert!(result[0][0].0.fg.is_some());
    assert!(result[1][0].0.fg.is_some());
}

#[test]
fn with_extra_syntaxes_recognizes_loaded_extension() {
    use crate::plugin::ExtraSyntax;

    // A minimal syntax that highlights "kw" as a keyword in .xtestlang files.
    // concat! keeps indentation correct (backslash continuations strip it).
    let syntax_yaml = concat!(
        "%YAML 1.2\n",
        "---\n",
        "name: TestLang\n",
        "file_extensions: [xtestlang]\n",
        "scope: source.xtestlang\n",
        "contexts:\n",
        "  main:\n",
        "    - match: 'kw'\n",
        "      scope: keyword.control.xtestlang\n",
    );
    let dir = tempfile::tempdir().unwrap();
    let syntax_path = dir.path().join("testlang.sublime-syntax");
    std::fs::write(&syntax_path, syntax_yaml).unwrap();

    let extra = vec![ExtraSyntax {
        syntax_path: syntax_path.clone(),
        extensions: vec![],
    }];
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &extra);
    // Extra syntax loaded → "kw rest" in a .xtestlang file should produce
    // multiple spans (keyword + trailing text), since the keyword rule fires.
    let result = h.highlight(Path::new("test.xtestlang"), &["kw rest".to_string()]);
    assert!(
        result[0].len() > 1,
        "extra syntax should produce multiple spans for .xtestlang files"
    );
    // Without the extra syntax, an unknown extension falls back to plain text
    // and returns a single unstyled span.
    let h2 = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result2 = h2.highlight(Path::new("test.xtestlang"), &["kw rest".to_string()]);
    assert_eq!(
        result2[0].len(),
        1,
        "unknown extension should produce one plain-text span"
    );
}
