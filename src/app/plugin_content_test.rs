//! Tests for plugin-rendered content installation and streaming.

use crate::app::App;
use crate::config::Config;

fn create_app() -> App {
    let root = tempfile::tempdir().unwrap();
    let app = App::new(root.path().to_path_buf(), Config::default(), None, None).unwrap();
    // The app retains the root path but does not need the temporary directory
    // after construction; content tests use synthetic paths.
    app
}

fn send(app: &mut App, name: &str, action: &str, params: serde_json::Value) {
    app.handle_plugin_action(name, action, &params);
}

// -- set_content tests --------------------------------------------------------

#[test]
fn set_content_stores_spans_and_text_for_path() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({
            "path": path.to_str().unwrap(),
            "lines": ["hello", "world"],
        }),
    );
    assert_eq!(app.plugin_content.get(&path).map(|l| l.len()), Some(2));
    assert_eq!(
        app.plugin_content_text.get(&path),
        Some(&vec!["hello".to_string(), "world".to_string()])
    );
}

#[test]
fn streamed_content_is_visible_incrementally_and_completes_after_the_terminator() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/streamed.md");
    app.current_file = Some(path.clone());
    let send = |app: &mut App, action: &str, params| {
        app.handle_plugin_action("md-plugin", action, &params);
    };

    send(
        &mut app,
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 0, "lines": ["first"]}),
    );
    assert_eq!(app.plugin_content_text.get(&path).unwrap(), &["first"]);
    assert!(app.plugin_content_streams.get(&path).unwrap().1.is_active());

    send(
        &mut app,
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 0, "lines": ["duplicate"]}),
    );
    send(
        &mut app,
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 2, "lines": ["third"]}),
    );
    assert_eq!(app.plugin_content_text.get(&path).unwrap(), &["first"]);
    send(
        &mut app,
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 1, "lines": ["second"]}),
    );
    assert_eq!(
        app.plugin_content_text.get(&path).unwrap(),
        &["first", "second", "third"]
    );
    send(
        &mut app,
        "set_content_end",
        serde_json::json!({"path": path, "content_id": "r1", "total_chunks": 3}),
    );
    assert!(!app.plugin_content_streams.get(&path).unwrap().1.is_active());
    assert!(!app
        .plugin_content_streams
        .get(&path)
        .unwrap()
        .1
        .is_incomplete());
}

#[test]
fn streamed_content_extends_an_active_filter_incrementally() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/filtered-stream.md");
    app.current_file = Some(path.clone());
    app.filter_query = Some("hit".into());

    send(
        &mut app,
        "md-plugin",
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 0, "lines": ["miss", "hit"]}),
    );
    assert_eq!(app.filter_display_map, vec![1]);
    send(
        &mut app,
        "md-plugin",
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 1, "lines": ["another hit"]}),
    );
    assert_eq!(app.filter_display_map, vec![1, 2]);
}

#[test]
fn stalled_stream_is_marked_incomplete_and_keeps_partial_content() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/stalled.md");
    app.current_file = Some(path.clone());
    send(
        &mut app,
        "md-plugin",
        "set_content_chunk",
        serde_json::json!({"path": path, "content_id": "r1", "index": 0, "lines": ["partial"]}),
    );
    let timeout = crate::plugin::STREAM_IDLE_TIMEOUT;
    app.expire_plugin_content_streams(std::time::Instant::now() + timeout);

    assert_eq!(app.plugin_content_text.get(&path).unwrap(), &["partial"]);
    assert!(app
        .plugin_content_streams
        .get(&path)
        .unwrap()
        .1
        .is_incomplete());
}

#[test]
fn set_content_for_current_file_resets_scroll_and_marks_active() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.current_file = Some(path.clone());
    app.content_scroll = 7;
    app.content_hscroll = 3;
    app.plugin_content_active = false;
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": path.to_str().unwrap(), "lines": ["x"]}),
    );
    assert_eq!(app.content_scroll, 0, "current-file render resets vscroll");
    assert_eq!(app.content_hscroll, 0, "current-file render resets hscroll");
    assert!(
        app.plugin_content_active,
        "current-file render marks active"
    );
}

#[test]
fn set_content_for_background_file_preserves_viewport() {
    // A plugin rendering a path other than the open file must not yank the
    // viewport of the file the user is currently reading.
    let mut app = create_app();
    app.current_file = Some(std::path::PathBuf::from("/tmp/open.md"));
    app.content_scroll = 7;
    app.content_hscroll = 3;
    app.plugin_content_active = false;
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/background.md", "lines": ["x"]}),
    );
    assert_eq!(
        app.content_scroll, 7,
        "background render must not reset vscroll"
    );
    assert_eq!(
        app.content_hscroll, 3,
        "background render must not reset hscroll"
    );
    assert!(
        !app.plugin_content_active,
        "background render must not mark active"
    );
    // Content is still stored for later use, keyed by its own path.
    assert!(app
        .plugin_content
        .contains_key(&std::path::PathBuf::from("/tmp/background.md")));
}

#[test]
fn set_content_preserves_scroll_on_same_path_re_render() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.current_file = Some(path.clone());
    // Set a content area so content_scroll_max() is meaningful
    app.content_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.content_scroll = 7;
    app.content_hscroll = 3;
    app.plugin_content_active = false;
    // First render resets scroll
    let many_lines: Vec<String> = (0..30).map(|i| format!("line{i}")).collect();
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": path.to_str().unwrap(), "lines": many_lines}),
    );
    assert_eq!(app.content_scroll, 0, "first render resets vscroll");
    assert_eq!(app.content_hscroll, 0, "first render resets hscroll");
    assert!(app.plugin_content_active, "first render marks active");
    // Set new scroll position
    app.content_scroll = 20;
    app.content_hscroll = 2;
    // Second render of same path must preserve scroll
    let many_lines: Vec<String> = (0..30).map(|i| format!("line{i}")).collect();
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": path.to_str().unwrap(), "lines": many_lines}),
    );
    assert_eq!(
        app.content_scroll, 20,
        "re-render of same path must preserve vscroll"
    );
    assert_eq!(
        app.content_hscroll, 2,
        "re-render of same path must preserve hscroll"
    );
}

#[test]
fn set_content_switching_path_resets_scroll() {
    let mut app = create_app();
    let first = std::path::PathBuf::from("/tmp/first.md");
    let second = std::path::PathBuf::from("/tmp/second.md");
    app.current_file = Some(first.clone());
    app.content_scroll = 7;
    app.plugin_content_active = false;
    // Render first file — resets scroll
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/first.md", "lines": ["a"]}),
    );
    assert_eq!(app.content_scroll, 0, "first render resets scroll");
    // Switch current file
    app.current_file = Some(second.clone());
    app.content_scroll = 3;
    app.plugin_content_active = false;
    // Render second file — resets scroll (different path from previous render)
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/second.md", "lines": ["b"]}),
    );
    assert_eq!(app.content_scroll, 0, "new current file resets scroll");
}

#[test]
fn set_content_same_path_preserves_scroll_after_file_reopen() {
    // When the file is re-opened (apply_file_load sets
    // plugin_content_active_path = None for new files), the next
    // set_content should be treated as a first render and reset scroll.
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.current_file = Some(path.clone());
    app.content_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    let many_lines: Vec<String> = (0..30).map(|i| format!("line{i}")).collect();
    // First render
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": many_lines}),
    );
    app.content_scroll = 20;
    // Simulate a same-file reload (apply_file_load preserves
    // plugin_content_active_path since same-file reload doesn't clear it)
    // The path remains current.
    let many_lines_b: Vec<String> = (0..30).map(|i| format!("other{i}")).collect();
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": many_lines_b}),
    );
    assert_eq!(
        app.content_scroll, 20,
        "re-render preserves scroll after same-file reload"
    );
}

#[test]
fn set_content_stamps_contribution() {
    let mut app = create_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    send(
        &mut app,
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": ["hello"]}),
    );
    let contrib = app.plugin_contributions.get("md-plugin").unwrap();
    assert!(
        contrib.content_paths.contains(&path),
        "content_paths must track the path"
    );
}
