use super::*;
use std::path::Path;

#[test]
fn detector_chosen_by_extension_case_insensitively() {
    assert_eq!(detector_for(Path::new("a.rs"), None), Some(Detector::Brace));
    assert_eq!(detector_for(Path::new("A.RS"), None), Some(Detector::Brace));
    assert_eq!(
        detector_for(Path::new("a.tsx"), None),
        Some(Detector::Brace)
    );
    assert_eq!(
        detector_for(Path::new("a.json"), None),
        Some(Detector::BraceBrackets)
    );
    assert_eq!(
        detector_for(Path::new("a.toml"), None),
        Some(Detector::Section)
    );
    assert_eq!(detector_for(Path::new("a.tf"), None), Some(Detector::Hcl));
    assert_eq!(detector_for(Path::new("a.sql"), None), Some(Detector::Sql));
    assert_eq!(
        detector_for(Path::new("a.j2"), None),
        Some(Detector::Template)
    );
    assert_eq!(
        detector_for(Path::new("a.py"), None),
        Some(Detector::Indent)
    );
}

#[test]
fn shebang_selects_shell_detector_only_without_a_known_extension() {
    assert_eq!(
        detector_for(Path::new("deploy"), Some("bash")),
        Some(Detector::Shell)
    );
    assert_eq!(detector_for(Path::new("deploy"), Some("python3")), None);
    assert_eq!(
        detector_for(Path::new("a.rs"), Some("bash")),
        Some(Detector::Brace)
    );
}

#[test]
fn yaml_and_unknown_files_have_no_dispatch_detector() {
    assert_eq!(detector_for(Path::new("a.yaml"), None), None);
    assert_eq!(detector_for(Path::new("notes.txt"), None), None);
    assert!(detect_for_path(Path::new("notes.txt"), None, "x").is_none());
}

#[test]
fn brace_detector_finds_multiline_block() {
    let regions =
        detect_for_path(Path::new("a.rs"), None, "fn main() {\n    let x = 1;\n}\n").unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!((regions[0].start, regions[0].end), (0, 2));
}

#[test]
fn json_detector_folds_arrays_too() {
    let regions = detect_for_path(
        Path::new("a.json"),
        None,
        "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n",
    )
    .unwrap();
    assert!(regions.iter().any(|r| r.start == 1 && r.end == 4));
}

#[test]
fn indent_detector_folds_python_def() {
    let regions = detect_for_path(Path::new("a.py"), None, "def f():\n    return 1\n").unwrap();
    assert_eq!(regions.len(), 1);
}

#[test]
fn toml_sections_fold() {
    let regions =
        detect_for_path(Path::new("a.toml"), None, "[a]\nx = 1\ny = 2\n[b]\nz = 3\n").unwrap();
    assert_eq!(regions.len(), 2);
}
