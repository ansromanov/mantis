use super::*;

fn hl() -> Highlighter {
    Highlighter::with_extra_syntaxes("base16-ocean.dark", &[])
}

#[test]
fn plain_file_uses_virtual_file() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"fn main() {}\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert!(load.ok);
    assert!(load.virtual_file.is_some());
    assert!(load.highlighted.is_empty());
    assert!(!load.is_json);
}

#[test]
fn json_produces_pretty_view() {
    let mut f = tempfile::NamedTempFile::with_suffix(".json").unwrap();
    use std::io::Write;
    f.write_all(br#"{"a":1,"b":[2,3]}"#).unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
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
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    let yaml = load.yaml.expect("yaml state");
    assert_eq!(yaml.anchor_count, 1);
    assert_eq!(yaml.alias_count, 1);
    assert!(!yaml.fold_regions.is_empty());
}

#[test]
fn missing_file_is_not_ok() {
    let load = compute_file_load(std::path::Path::new("/no/such/file.txt"), &hl(), usize::MAX);
    assert!(!load.ok);
    assert!(load.content[0].starts_with("[error:"));
}

#[test]
fn empty_file_message() {
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.content, vec!["[empty file]".to_string()]);
}

#[test]
fn ascii_vf_path_sets_encoding_and_line_ending() {
    // .rs extension → VirtualFile path
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"fn main() {}\nlet x = 1;\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.encoding.as_deref(), Some("ASCII"));
    assert_eq!(load.line_ending.as_deref(), Some("LF"));
}

#[test]
fn utf8_bom_fallback_path_sets_encoding() {
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"\xEF\xBB\xBFhello\nworld\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.encoding.as_deref(), Some("UTF-8 BOM"));
}

#[test]
fn crlf_content_is_split_and_stripped() {
    let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    use std::io::Write;
    f.write_all(b"line one\r\nline two\r\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.line_ending.as_deref(), Some("CRLF"));
    let vf = load.virtual_file.expect("virtual file");
    assert_eq!(vf.line_text(0), Some("line one"));
    assert_eq!(vf.line_text(1), Some("line two"));
}

#[test]
fn binary_file_sets_binary_encoding() {
    // NUL byte → VirtualFile rejects, fallback detects binary
    let mut f = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    use std::io::Write;
    f.write_all(b"data\x00binary").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.encoding.as_deref(), Some("BINARY"));
    assert_eq!(load.content, vec!["[binary file]"]);
}

#[test]
fn worker_round_trip_returns_matching_seq() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"let x = 1;\n").unwrap();
    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    loader.request(LoadRequest::File {
        seq: 7,
        path: f.path().to_path_buf(),
    });
    let resp = loader.rx.recv().expect("worker response");
    match resp {
        LoadResponse::File { seq, load, .. } => {
            assert_eq!(seq, 7);
            assert!(load.ok);
        }
        _ => panic!("expected File response"),
    }
}

#[test]
fn worker_echoes_barrier_after_prior_requests_are_applied() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"let x = 1;\n").unwrap();
    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    loader.request(LoadRequest::File {
        seq: 3,
        path: f.path().to_path_buf(),
    });
    loader.request(LoadRequest::Barrier(42));
    // Since the request channel is FIFO and single-threaded, the File
    // response must be observed before the Barrier echo.
    match loader.rx.recv().expect("worker response") {
        LoadResponse::File { seq, .. } => assert_eq!(seq, 3),
        _ => panic!("expected File response before the barrier echo"),
    }
    match loader.rx.recv().expect("worker response") {
        LoadResponse::Barrier(token) => assert_eq!(token, 42),
        _ => panic!("expected Barrier echo"),
    }
}

#[test]
fn worker_rebuilds_highlighter_on_set_extra_syntaxes_and_keeps_serving() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"let x = 1;\n").unwrap();
    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    // Push an updated syntax set; the worker must rebuild its highlighter and
    // continue to process file loads (the SetExtraSyntaxes match arm).
    loader.request(LoadRequest::SetExtraSyntaxes(Vec::new()));
    loader.request(LoadRequest::File {
        seq: 9,
        path: f.path().to_path_buf(),
    });
    let resp = loader.rx.recv().expect("worker response");
    match resp {
        LoadResponse::File { seq, load, .. } => {
            assert_eq!(seq, 9);
            assert!(load.ok);
        }
        _ => panic!("expected File response"),
    }
}

#[test]
fn compute_file_load_sets_syntax_name_for_rust_file() {
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    use std::io::Write;
    f.write_all(b"fn main() {}\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.syntax_name.as_deref(), Some("Rust"));
}

#[test]
fn git_status_worker_round_trip_returns_matching_seq() {
    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    loader.request(LoadRequest::GitStatus {
        seq: 42,
        root: std::path::PathBuf::from("/nonexistent"),
        include_untracked: true,
        include_ignored: false,
    });
    let resp = loader.rx.recv().expect("worker response");
    match resp {
        LoadResponse::GitStatus { seq, root, load } => {
            assert_eq!(seq, 42);
            assert_eq!(root, std::path::PathBuf::from("/nonexistent"));
            // Outside a git repo both should be empty.
            assert!(load.status_map.is_empty());
            assert!(load.info.is_none());
        }
        _ => panic!("expected GitStatus response"),
    }
}

#[test]
fn compute_git_status_load_outside_repo() {
    let load = compute_git_status_load(std::path::Path::new("/nonexistent"), true, false);
    assert!(load.status_map.is_empty());
    assert!(load.info.is_none());
}

#[test]
fn compute_file_load_sets_no_syntax_name_for_unknown_extension() {
    let mut f = tempfile::NamedTempFile::with_suffix(".zzunknown").unwrap();
    use std::io::Write;
    f.write_all(b"hello world\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.syntax_name, None);
}
