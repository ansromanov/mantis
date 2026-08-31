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
fn credential_file_is_masked_before_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(".env");
    let mut f = std::fs::File::create(&path).unwrap();
    use std::io::Write;
    f.write_all(b"API_TOKEN=super-secret\nNAME=mantis\n")
        .unwrap();
    let load = compute_file_load(&path, &hl(), usize::MAX);
    assert!(load.secret_masked);
    assert_eq!(load.content[0], "API_TOKEN=********");
    assert_eq!(load.secret_original[0], "API_TOKEN=super-secret");
}

#[test]
fn ordinary_file_with_secret_shape_is_not_memory_mapped() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(file.path(), "SERVICE_TOKEN=secret-value\n").unwrap();
    let load = compute_file_load(file.path(), &hl(), usize::MAX);
    assert!(load.secret_masked);
    assert_eq!(load.content, vec!["SERVICE_TOKEN=********"]);
    assert_eq!(load.secret_original, vec!["SERVICE_TOKEN=secret-value"]);
    assert!(load.virtual_file.is_none());
}

#[test]
fn pem_file_masks_the_private_key_body() {
    let file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(
        file.path(),
        format!(
            "-----BEGIN {marker}\nprivate-key-body\n-----END {marker}\n",
            marker = "PRIVATE KEY-----"
        ),
    )
    .unwrap();
    let load = compute_file_load(file.path(), &hl(), usize::MAX);
    assert_eq!(load.content, vec!["********", "********", "********"]);
}

#[test]
fn jsonl_loads_collapsed_rows() {
    let mut f = tempfile::NamedTempFile::with_suffix(".jsonl").unwrap();
    use std::io::Write;
    f.write_all(
        br#"{"level":"info","message":"ready"}
{"level":"error","message":"failed"}
"#,
    )
    .unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert!(load.is_jsonl);
    assert_eq!(load.content.len(), 2);
    assert!(load.content[0].contains("level=\"info\""));
    assert_eq!(load.jsonl_source.len(), 2);
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
    assert_eq!(
        load.content,
        vec![
            "[binary file — TXT file, 11 B]".to_string(),
            "".to_string(),
            "press o to open with the system default app".to_string()
        ]
    );
}

fn png_1x1() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(1, 1, image::Rgb([1, 2, 3]));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
        .unwrap();
    buf
}

#[test]
fn compute_file_load_builds_image_placeholder_and_leaves_inline_image_unset() {
    // The test process never runs `graphics::detect::detect`, so Kitty support
    // stays at its cached default (false); `image` is expected to stay unset
    // even though the bytes are a real, decodable PNG.
    let mut f = tempfile::NamedTempFile::with_suffix(".png").unwrap();
    use std::io::Write;
    f.write_all(&png_1x1()).unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert_eq!(load.encoding.as_deref(), Some("BINARY"));
    assert!(load.content[0].starts_with("[image file — PNG, 1x1,"));
    assert!(load.image.is_none());
}

#[test]
fn decode_inline_image_is_none_without_graphics_support() {
    assert!(decode_inline_image(std::path::Path::new("pixel.png"), &png_1x1()).is_none());
}

#[test]
fn decode_inline_image_is_none_for_non_image_bytes() {
    assert!(decode_inline_image(std::path::Path::new("pixel.png"), b"not an image").is_none());
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
fn range_status_worker_round_trip_reports_error_outside_repo() {
    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    loader.request(LoadRequest::RangeStatus {
        seq: 7,
        root: std::path::PathBuf::from("/nonexistent"),
        rev: "HEAD".to_string(),
    });
    let resp = loader.rx.recv().expect("worker response");
    match resp {
        LoadResponse::RangeStatus {
            seq,
            root,
            load,
            error,
        } => {
            assert_eq!(seq, 7);
            assert_eq!(root, std::path::PathBuf::from("/nonexistent"));
            assert!(load.status_map.is_empty());
            assert!(error.is_some(), "not a git repo should surface an error");
        }
        _ => panic!("expected RangeStatus response"),
    }
}

#[test]
fn range_status_worker_round_trip_succeeds_in_real_repo() {
    let dir = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(dir.path())
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    std::fs::write(dir.path().join("f.txt"), "v1\n").unwrap();
    git(&["add", "f.txt"]);
    git(&["commit", "-q", "-m", "init"]);
    std::fs::write(dir.path().join("f.txt"), "v2\n").unwrap();

    let loader = Loader::new(&Theme::default(), Vec::new(), usize::MAX);
    loader.request(LoadRequest::RangeStatus {
        seq: 3,
        root: dir.path().to_path_buf(),
        rev: "HEAD".to_string(),
    });
    let resp = loader.rx.recv().expect("worker response");
    match resp {
        LoadResponse::RangeStatus {
            seq, load, error, ..
        } => {
            assert_eq!(seq, 3);
            assert!(error.is_none());
            let root = dir.path().canonicalize().unwrap();
            assert_eq!(
                load.status_map.get(&root.join("f.txt")),
                Some(&crate::git::GitStatus::Modified)
            );
        }
        _ => panic!("expected RangeStatus response"),
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

#[test]
fn compute_file_load_csv_builds_table() {
    let mut f = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
    use std::io::Write;
    f.write_all(b"name,age\nAlice,30\nBob,25\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert!(load.is_csv);
    assert!(load.show_csv_table);
    assert!(!load.csv_table_text.is_empty());
    assert!(!load.csv_table_lines.is_empty());
    assert!(load.csv_table_text[0].contains('┌'));
    assert!(load.csv_table_text[1].contains("name"));
    assert!(load.csv_table_text[1].contains("age"));
}

#[test]
fn compute_file_load_tsv_builds_table() {
    let mut f = tempfile::NamedTempFile::with_suffix(".tsv").unwrap();
    use std::io::Write;
    f.write_all(b"colA\tcolB\n1\t2\n").unwrap();
    let load = compute_file_load(f.path(), &hl(), usize::MAX);
    assert!(load.is_csv);
    assert!(load.show_csv_table);
    assert!(!load.csv_table_text.is_empty());
    assert!(load.csv_table_text[1].contains("colA"));
    assert!(load.csv_table_text[1].contains("colB"));
}

#[test]
fn compute_file_load_csv_exceeding_prettify_limit_falls_back() {
    let mut f = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
    use std::io::Write;
    f.write_all(b"a,b,c\n1,2,3\n").unwrap();
    // Set size limit to 3 bytes (smaller than the file)
    let load = compute_file_load(f.path(), &hl(), 3);
    assert!(load.is_csv);
    assert!(load.prettify_size_limit_exceeded);
    assert!(!load.show_csv_table);
    assert!(load.virtual_file.is_some() || !load.content.is_empty());
}
