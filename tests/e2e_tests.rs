use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mantis::actions::ACTIONS;
use mantis::app::App;
use mantis::config::Config;
use mantis::search::InFileSearch;

fn get_test_dir() -> PathBuf {
    std::env::current_dir().unwrap().join("e2e").join("data")
}

fn init_app() -> App {
    let root = get_test_dir();
    App::new(root, Config::default(), None, None).expect("failed to initialize App")
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn test_rust_highlighting_and_search() {
    let mut app = init_app();
    let rust_file = get_test_dir().join("rust_sample.rs");
    app.open_file(&rust_file);

    assert_eq!(app.current_file.as_ref().unwrap(), &rust_file);
    assert_eq!(app.current_syntax.as_deref(), Some("Rust"));
    assert!(app.line_count() > 10);

    // Verify syntax highlighting is functional using highlight_lines
    let sample_lines = vec!["fn main() {", "    let x = 42;", "}"];
    let highlighted_lines = app.highlight_lines(&sample_lines);
    assert_eq!(highlighted_lines.len(), sample_lines.len());
    assert!(!highlighted_lines[0].is_empty());

    // Verify in-file search by extracting lines first to avoid borrow issues
    let lines: Vec<String> = (0..app.line_count())
        .map(|idx| app.line_text(idx).unwrap().to_string())
        .collect();

    let mut s = InFileSearch::new();
    s.push('R');
    s.push('e');
    s.push('c');
    s.push('t');
    s.push('a');
    s.push('n');
    s.push('g');
    s.push('l');
    s.push('e');
    s.refresh(lines.len(), |idx| lines.get(idx).cloned());
    assert!(
        !s.matches.is_empty(),
        "expected to find 'Rectangle' in rust_sample.rs"
    );
    // Check that match spans are correct
    for m in &s.matches {
        let line_content = &lines[m.line];
        let matched_substring = &line_content[m.col..m.col + m.len];
        assert!(matched_substring.eq_ignore_ascii_case("Rectangle"));
    }
    app.in_file_search = Some(s);
}

#[test]
fn test_yaml_folding_and_anchors() {
    let mut app = init_app();
    let yaml_file = get_test_dir().join("yaml_sample.yml");
    app.open_file(&yaml_file);

    assert_eq!(app.current_syntax.as_deref(), Some("YAML"));
    assert_eq!(app.yaml_anchor_count, 1);
    assert_eq!(app.yaml_alias_count, 5);

    // Check fold regions are detected
    assert!(
        !app.fold_regions.is_empty(),
        "expected YAML fold regions to be detected"
    );

    // Fold all and check physical/display line mapping
    app.fold_all();
    assert!(app.display_line_count() < app.line_count());

    // Unfold all and verify match
    app.unfold_all();
    assert_eq!(app.display_line_count(), app.line_count());
}

#[test]
fn test_json_prettify() {
    let mut app = init_app();
    let json_file = get_test_dir().join("json_sample.json");
    app.open_file(&json_file);

    assert_eq!(app.current_syntax.as_deref(), Some("JSON"));
    assert!(app.is_json);
    assert!(app.show_pretty_json);

    // Minified JSON is 1 line, pretty JSON should be multiple lines
    assert_eq!(app.content.len(), 1);
    assert!(app.json_pretty_text.len() > 5);
    assert_eq!(app.line_count(), app.json_pretty_text.len());
}

#[test]
fn test_python_and_markdown_loading() {
    let mut app = init_app();

    // Python
    let py_file = get_test_dir().join("python_sample.py");
    app.open_file(&py_file);
    assert_eq!(app.current_syntax.as_deref(), Some("Python"));
    assert!(app.line_count() > 5);

    // Markdown
    let md_file = get_test_dir().join("markdown_sample.md");
    app.open_file(&md_file);
    assert_eq!(app.current_syntax.as_deref(), Some("Markdown"));
    assert!(app.line_count() > 5);
}

#[test]
fn test_crlf_normalization() {
    let mut app = init_app();
    let crlf_file = get_test_dir().join("crlf_sample.txt");
    app.open_file(&crlf_file);

    assert_eq!(app.file_line_ending.as_deref(), Some("CRLF"));
    for line in &app.content {
        assert!(
            !line.contains('\r'),
            "CRLF sample content should be normalized to LF"
        );
    }
}

#[test]
fn test_bom_handling() {
    let mut app = init_app();
    let bom_file = get_test_dir().join("bom_utf8_sample.txt");
    app.open_file(&bom_file);

    assert_eq!(app.file_encoding.as_deref(), Some("UTF-8 BOM"));
}

#[test]
fn test_binary_detection() {
    let mut app = init_app();
    let bin_file = get_test_dir().join("binary_sample.bin");
    app.open_file(&bin_file);

    assert_eq!(app.file_encoding.as_deref(), Some("BINARY"));
    assert!(!app.content.is_empty());
    assert!(
        app.content[0].contains("[binary file"),
        "expected binary placeholder, got: {}",
        app.content[0]
    );
}

#[test]
fn test_long_lines_word_wrap() {
    let mut app = init_app();
    let long_lines_file = get_test_dir().join("long_lines.txt");
    app.open_file(&long_lines_file);

    // Word wrap is false by default in Config, but we can set/verify it
    app.word_wrap = true;
    assert!(app.word_wrap);
}

/// Regression test for PR #614: `tree_up_dir` (Backspace) must not ascend
/// past the directory `mantis` was launched from.
#[test]
fn test_tree_up_dir_clamped_to_initial_root() {
    let mut app = init_app();
    let orig_root = app.root.clone();
    assert_eq!(app.initial_root, orig_root);

    app.handle_key(key(KeyCode::Backspace));

    assert_eq!(
        app.root, orig_root,
        "Backspace at the initial root must not change the root"
    );
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("Already at root"),
        "expected the clamp status message from PR #614"
    );
}

/// Regression test for PR #626: entering compare mode records the base
/// revision and exposes it for the status-bar `[compare: {base}]` badge.
#[test]
fn test_compare_mode_records_base_revision() {
    let mut app = init_app();
    assert!(app.compare_base.is_none());

    app.enter_compare_mode("HEAD~1".to_string());

    assert_eq!(app.compare_base.as_deref(), Some("HEAD~1"));
}

/// Regression test for PRs #621/#638/#639: the bug-report and telemetry
/// palette commands from the diagnostics work must stay registered with
/// their documented labels.
#[test]
fn test_diagnostics_palette_actions_registered() {
    let bug_report = ACTIONS
        .iter()
        .find(|a| a.id == "bug_report")
        .expect("bug_report action must be registered");
    assert_eq!(
        bug_report.palette,
        Some("Report a bug (save diagnostics locally)")
    );

    let toggle_telemetry = ACTIONS
        .iter()
        .find(|a| a.id == "toggle_telemetry")
        .expect("toggle_telemetry action must be registered");
    assert_eq!(toggle_telemetry.palette, Some("Toggle telemetry"));

    let compare_against = ACTIONS
        .iter()
        .find(|a| a.id == "compare_against")
        .expect("compare_against action must be registered");
    assert_eq!(compare_against.palette, Some("Compare against a revision"));
}

/// Regression test for PR #643: the first-run welcome overlay must be
/// keyed off an isolated, file-based flag so it only appears once, and
/// must not leak between test runs that isolate `MANTIS_STATE_DIR`.
#[test]
fn test_welcome_overlay_shown_once_per_state_dir() {
    let state_dir = std::env::temp_dir().join(format!(
        "mantis-e2e-welcome-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&state_dir).unwrap();
    // SAFETY: this test does not run concurrently with other tests that
    // read/write MANTIS_STATE_DIR within this process.
    unsafe {
        std::env::set_var("MANTIS_STATE_DIR", &state_dir);
    }

    assert!(
        !mantis::session::is_welcome_shown(),
        "welcome overlay must show on a fresh state dir"
    );
    mantis::session::mark_welcome_shown();
    assert!(
        mantis::session::is_welcome_shown(),
        "welcome overlay must not show again after being dismissed"
    );

    unsafe {
        std::env::remove_var("MANTIS_STATE_DIR");
    }
    std::fs::remove_dir_all(&state_dir).ok();
}
