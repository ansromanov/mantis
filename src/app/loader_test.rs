use super::*;
use crate::theme::Theme;

fn hl() -> Highlighter {
    Highlighter::with_extra_syntaxes("base16-ocean.dark", &[])
}

#[test]
fn plain_file_uses_virtual_file() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"fn main() {}\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert!(load.ok);
    assert!(load.virtual_file.is_some());
    assert!(load.highlighted.is_empty());
    assert!(!load.is_markdown && !load.is_json);
}

#[test]
fn markdown_renders_lines() {
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"# Title\n\nbody\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert!(load.is_markdown);
    assert!(load.virtual_file.is_none());
    assert!(!load.markdown_lines.is_empty());
}

#[test]
fn json_produces_pretty_view() {
    let mut f = tempfile::NamedTempFile::with_suffix(".json").unwrap();
    use std::io::Write;
    f.write_all(br#"{"a":1,"b":[2,3]}"#).unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert!(load.is_json);
    assert!(load.show_pretty_json);
    assert!(!load.json_pretty_text.is_empty());
    assert!(!load.json_pretty_lines.is_empty());
}

#[test]
fn yaml_detects_folds_and_anchors() {
    let mut f = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
    use std::io::Write;
    f.write_all(b"root: &a\n  key: val\nref: *a\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    let yaml = load.yaml.expect("yaml state");
    assert_eq!(yaml.anchor_count, 1);
    assert_eq!(yaml.alias_count, 1);
    assert!(!yaml.fold_regions.is_empty());
}

#[test]
fn missing_file_is_not_ok() {
    let load = compute_file_load(
        std::path::Path::new("/no/such/file.txt"),
        &Theme::default(),
        &hl(),
    );
    assert!(!load.ok);
    assert!(load.content[0].starts_with("[error:"));
}

#[test]
fn empty_file_message() {
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert_eq!(load.content, vec!["[empty file]".to_string()]);
}

#[test]
fn ascii_vf_path_sets_encoding_and_line_ending() {
    // .rs extension → VirtualFile path
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"fn main() {}\nlet x = 1;\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert_eq!(load.encoding.as_deref(), Some("ASCII"));
    assert_eq!(load.line_ending.as_deref(), Some("LF"));
}

#[test]
fn utf8_bom_fallback_path_sets_encoding() {
    // .md extension → fallback path (markdown rendering)
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"\xEF\xBB\xBFhello\nworld\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert_eq!(load.encoding.as_deref(), Some("UTF-8 BOM"));
}

#[test]
fn crlf_content_is_split_and_stripped() {
    // .md extension → fallback path; CRLF must be normalized before splitting
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"line one\r\nline two\r\n").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert_eq!(load.line_ending.as_deref(), Some("CRLF"));
    assert_eq!(load.content, vec!["line one", "line two"]);
}

#[test]
fn binary_file_sets_binary_encoding() {
    // NUL byte → VirtualFile rejects, fallback detects binary
    let mut f = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    use std::io::Write;
    f.write_all(b"data\x00binary").unwrap();
    let load = compute_file_load(f.path(), &Theme::default(), &hl());
    assert_eq!(load.encoding.as_deref(), Some("BINARY"));
    assert_eq!(load.content, vec!["[binary file]"]);
}

#[test]
fn worker_round_trip_returns_matching_seq() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"let x = 1;\n").unwrap();
    let loader = Loader::new(&Theme::default(), Vec::new());
    loader.request(LoadRequest::File {
        seq: 7,
        path: f.path().to_path_buf(),
    });
    let resp = loader
        .rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .expect("worker response");
    match resp {
        LoadResponse::File { seq, load, .. } => {
            assert_eq!(seq, 7);
            assert!(load.ok);
        }
        _ => panic!("expected File response"),
    }
}
