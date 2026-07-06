use super::*;
use std::io::Write;
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

#[test]
fn syntax_name_returns_known_language() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    f.write_all(b"fn main() {}\n").unwrap();
    let name = h.syntax_name(f.path());
    assert_eq!(name.as_deref(), Some("Rust"));
}

#[test]
fn syntax_name_returns_none_for_unknown_extension() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let mut f = tempfile::NamedTempFile::with_suffix(".zzunknown").unwrap();
    f.write_all(b"hello\n").unwrap();
    let name = h.syntax_name(f.path());
    assert_eq!(name, None);
}

#[test]
fn highlight_range_uses_resolved_syntax_name_not_a_path() {
    // highlight_range takes the syntax name resolved once at file-open time
    // (see `syntax_name`) instead of re-detecting it from a path on every call.
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight_range(Some("Rust"), &["fn main() {"]);
    assert!(
        result[0].len() > 1,
        "expected multiple spans for Rust code, got {}",
        result[0].len()
    );
}

#[test]
fn highlight_range_none_syntax_name_falls_back_to_plain_text() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight_range(None, &["fn main() {"]);
    assert_eq!(
        result[0].len(),
        1,
        "no syntax name should produce one plain-text span"
    );
}

#[test]
fn highlight_range_unknown_syntax_name_falls_back_to_plain_text() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight_range(Some("NotARealSyntax"), &["fn main() {"]);
    assert_eq!(
        result[0].len(),
        1,
        "unresolvable syntax name should produce one plain-text span"
    );
}

#[test]
fn highlight_stdin_uses_explicit_language_token() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["fn main() {".to_string()];
    let (spans, name) = h.highlight_stdin(Some("rust"), &lines);
    assert_eq!(name.as_deref(), Some("Rust"));
    assert!(
        spans[0].len() > 1,
        "expected multiple spans for Rust code, got {}",
        spans[0].len()
    );
}

#[test]
fn highlight_stdin_falls_back_to_first_line_sniffing() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["#!/usr/bin/env python3".to_string(), "print(1)".to_string()];
    let (_, name) = h.highlight_stdin(None, &lines);
    assert_eq!(name.as_deref(), Some("Python"));
}

#[test]
fn highlight_stdin_unknown_language_falls_back_to_sniffing() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["#!/usr/bin/env python3".to_string()];
    let (_, name) = h.highlight_stdin(Some("not-a-real-language"), &lines);
    assert_eq!(name.as_deref(), Some("Python"));
}

#[test]
fn highlight_stdin_plain_text_returns_none_name() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let lines = vec!["just some plain text".to_string()];
    let (spans, name) = h.highlight_stdin(None, &lines);
    assert_eq!(name, None);
    assert_eq!(spans[0].len(), 1);
}

#[test]
fn to_ratatui_respects_no_color() {
    std::env::set_var("MANTIS_TEST_NO_COLOR", "1");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let result = h.highlight(Path::new("main.rs"), &["fn main() {".to_string()]);
    assert!(result[0].len() > 1);
    for (style, _) in &result[0] {
        assert!(
            style.fg.is_none(),
            "no foreground color should be set when NO_COLOR is active"
        );
    }
    std::env::remove_var("MANTIS_TEST_NO_COLOR");
}
