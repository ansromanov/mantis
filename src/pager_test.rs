use super::*;

// ---------------------------------------------------------------------------
// parse_pager_bytes: binary / empty
// ---------------------------------------------------------------------------

#[test]
fn parse_pager_bytes_detects_binary() {
    let bytes = b"hello\0world";
    let parsed = parse_pager_bytes(bytes);
    // Matches the file-load convention (`compute_file_load` in `app::loader`)
    // so binary input reads the same whether it came from a path or a pipe.
    assert_eq!(parsed.content, vec!["[binary file]".to_string()]);
    assert!(!parsed.is_diff);
}

#[test]
fn parse_pager_bytes_empty_input() {
    let parsed = parse_pager_bytes(b"");
    assert_eq!(parsed.content, vec!["[empty file]".to_string()]);
    assert!(!parsed.is_diff);
}

// ---------------------------------------------------------------------------
// parse_pager_bytes: line splitting / CRLF normalization
// ---------------------------------------------------------------------------

#[test]
fn parse_pager_bytes_splits_lines() {
    let parsed = parse_pager_bytes(b"one\ntwo\nthree\n");
    assert_eq!(parsed.content, vec!["one", "two", "three"]);
}

#[test]
fn parse_pager_bytes_normalizes_crlf() {
    let parsed = parse_pager_bytes(b"one\r\ntwo\r\n");
    assert_eq!(parsed.content, vec!["one", "two"]);
}

// ---------------------------------------------------------------------------
// parse_pager_bytes: diff sniffing
// ---------------------------------------------------------------------------

#[test]
fn parse_pager_bytes_detects_git_diff_header() {
    let input = b"diff --git a/foo.rs b/foo.rs\nindex 123..456 100644\n--- a/foo.rs\n+++ b/foo.rs\n@@ -1,2 +1,2 @@\n-old\n+new\n";
    let parsed = parse_pager_bytes(input);
    assert!(parsed.is_diff);
}

#[test]
fn parse_pager_bytes_detects_plain_unified_diff() {
    // `diff -u a b` output has no `diff --git` line, just the file markers
    // followed by a hunk header.
    let input = b"--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new\n";
    let parsed = parse_pager_bytes(input);
    assert!(parsed.is_diff);
}

#[test]
fn parse_pager_bytes_hunk_header_alone_is_diff() {
    let input = b"@@ -1,3 +1,3 @@\n context\n-old\n+new\n";
    let parsed = parse_pager_bytes(input);
    assert!(parsed.is_diff);
}

#[test]
fn parse_pager_bytes_plain_text_is_not_diff() {
    let input = b"fn main() {\n    println!(\"hi\");\n}\n";
    let parsed = parse_pager_bytes(input);
    assert!(!parsed.is_diff);
}

#[test]
fn parse_pager_bytes_markdown_frontmatter_is_not_diff() {
    // A bare `---` (YAML front matter / markdown rule) must not be
    // misdetected as a diff file marker without a following `+++ `.
    let input = b"---\ntitle: hello\n---\n\n# Heading\n";
    let parsed = parse_pager_bytes(input);
    assert!(!parsed.is_diff);
}

#[test]
fn parse_pager_bytes_diff_marker_outside_sniff_window_is_ignored() {
    let mut input = String::new();
    for i in 0..(DIFF_SNIFF_LINES + 5) {
        input.push_str(&format!("line {i}\n"));
    }
    input.push_str("diff --git a/foo b/foo\n");
    let parsed = parse_pager_bytes(input.as_bytes());
    assert!(!parsed.is_diff);
}
