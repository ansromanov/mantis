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
    // See NO_COLOR_TEST_ENV_LOCK: serializes against other tests reading
    // no_color_active() while we flip the shared test-only env var.
    let _guard = crate::theme::lock_no_color_test_env();
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

#[test]
fn highlight_range_empty_lines() {
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[]);
    let spans = h.highlight_range(Some("Rust"), &[]);
    assert!(spans.is_empty());
}

fn extra_syntax(name: &str) -> ExtraSyntax {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("plugins")
        .join(name)
        .join("syntaxes")
        .join(format!("{name}.sublime-syntax"));
    ExtraSyntax {
        syntax_path: path,
        extensions: vec![],
    }
}

#[test]
fn syntax_name_resolves_toml_extension() {
    let extra = extra_syntax("toml");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    let mut f = tempfile::NamedTempFile::with_suffix(".toml").unwrap();
    f.write_all(b"[section]\nkey = \"value\"\n").unwrap();
    let name = h.syntax_name(f.path());
    assert_eq!(name.as_deref(), Some("TOML"));
}

#[test]
fn highlight_toml_highlights_sections() {
    let extra = extra_syntax("toml");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    let result = h.highlight(Path::new("config.toml"), &["[dependencies]".to_string()]);
    assert!(
        result[0].len() > 1,
        "expected multiple spans for TOML section header"
    );
}

#[test]
fn syntax_name_resolves_typescript_extensions() {
    let extra = extra_syntax("typescript");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    for suffix in &[".ts", ".tsx", ".mts", ".cts", ".jsx"] {
        let mut f = tempfile::NamedTempFile::with_suffix(suffix).unwrap();
        f.write_all(b"const x: number = 1;\n").unwrap();
        let name = h.syntax_name(f.path());
        assert_eq!(
            name.as_deref(),
            Some("TypeScript"),
            "expected TypeScript for extension {suffix}"
        );
    }
}

#[test]
fn highlight_typescript_highlights_keywords() {
    let extra = extra_syntax("typescript");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    let result = h.highlight(
        Path::new("main.ts"),
        &["function greet(name: string) {}".to_string()],
    );
    assert!(
        result[0].len() > 1,
        "expected multiple spans for TypeScript code"
    );
}

#[test]
fn syntax_name_resolves_dockerfile_by_filename() {
    let extra = extra_syntax("dockerfile");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    // Create a file named "Dockerfile" (no extension) in a temp dir.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Dockerfile");
    std::fs::write(&path, "FROM ubuntu:22.04\nRUN apt-get update\n").unwrap();
    let name = h.syntax_name(&path);
    assert_eq!(name.as_deref(), Some("Dockerfile"));
}

#[test]
fn syntax_name_resolves_containerfile_by_filename() {
    let extra = extra_syntax("dockerfile");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Containerfile");
    // First line is a comment, so first-line sniffing (`^FROM`) can't rescue
    // it — only the Containerfile → Dockerfile filename alias resolves this.
    std::fs::write(&path, "# syntax=docker/dockerfile:1\nFROM alpine\n").unwrap();
    let name = h.syntax_name(&path);
    assert_eq!(name.as_deref(), Some("Dockerfile"));
}

#[test]
fn highlight_tsx_styles_jsx_tag_name() {
    let extra = extra_syntax("typescript");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    let result = h.highlight(Path::new("app.tsx"), &["<div>".to_string()]);
    let plain = h.highlight(Path::new("noext"), &["div".to_string()]);
    let div_span = result[0]
        .iter()
        .find(|(_, text)| text == "div")
        .expect("expected a distinct span for the JSX tag name");
    assert_ne!(
        div_span.0, plain[0][0].0,
        "JSX tag name must be styled, not plain text — the jsx context \
         must win over the operators context for `<`"
    );
}

#[test]
fn highlight_dockerfile_highlights_instructions() {
    let extra = extra_syntax("dockerfile");
    let h = Highlighter::with_extra_syntaxes("base16-ocean.dark", &[extra]);
    // highlight() uses find_syntax_for_file which tries extensions first,
    // then first-line matching. The Dockerfile syntax has first_line_match
    // for FROM, so even without a filename-based lookup, highlight() should
    // detect it via first-line sniffing.
    let result = h.highlight(Path::new("Dockerfile"), &["FROM ubuntu:22.04".to_string()]);
    assert!(
        result[0].len() > 1,
        "expected multiple spans for Dockerfile FROM instruction"
    );
}
