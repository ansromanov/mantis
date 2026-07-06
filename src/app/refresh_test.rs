use std::collections::HashSet;
use std::fs;
use std::time::{Duration, Instant};

use super::*;
use crate::app::loader::compute_file_load;
use crate::app::StatusMessage;

// -- debounce / tick tests ----------------------------------------------------

#[test]
fn tick_debounce_holds_while_not_quiet() {
    let mut app = create_base_app();
    app.tree_dirty = true;
    app.tree_dirty_at = Some(Instant::now());
    app.tick();
    // TREE_RELOAD_DEBOUNCE is 60 s in test builds; a freshly set timestamp
    // cannot have elapsed, so the dirty flag must remain.
    assert!(
        app.tree_dirty,
        "debounce must keep tree_dirty until quiet period elapses"
    );
}

#[test]
fn tick_debounce_clears_dirty_after_quiet_period() {
    let mut app = create_base_app();
    app.tree_dirty = true;
    app.tree_dirty_at = Some(Instant::now() - Duration::from_secs(61));
    app.tick();
    // Quiet period has elapsed; tick() should reload and clear the flag.
    assert!(!app.tree_dirty, "tree_dirty must be cleared after reload");
    assert!(
        app.tree_dirty_at.is_none(),
        "tree_dirty_at must be cleared after reload"
    );
}

// -- set_icon_map action tests ------------------------------------------------

#[test]
fn set_icon_map_populates_icon_fields() {
    let mut app = App {
        icon_map: std::collections::HashMap::new(),
        icon_dir_open: String::new(),
        icon_dir_closed: String::new(),
        icon_fallback: String::new(),
        icons_enabled: false,
        ..create_base_app()
    };

    let params = serde_json::json!({
        "dir_open": "\u{f07c}",
        "dir_closed": "\u{f07b}",
        "fallback": "\u{f15b}",
        "icons": {
            "rs": "\u{e7a8}",
            "py": "\u{e73c}",
            "js": "\u{e74e}"
        }
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    assert!(app.icons_enabled, "set_icon_map must enable icons");
    assert_eq!(app.icon_dir_open, "\u{f07c}");
    assert_eq!(app.icon_dir_closed, "\u{f07b}");
    assert_eq!(app.icon_fallback, "\u{f15b}");
    assert_eq!(app.icon_map.get("rs"), Some(&"\u{e7a8}".to_string()));
    assert_eq!(app.icon_map.get("py"), Some(&"\u{e73c}".to_string()));
    assert_eq!(app.icon_map.get("js"), Some(&"\u{e74e}".to_string()));
}

#[test]
fn set_icon_map_stores_keys_lowercase() {
    let mut app = App {
        icon_map: std::collections::HashMap::new(),
        icon_dir_open: String::new(),
        icon_dir_closed: String::new(),
        icon_fallback: String::new(),
        icons_enabled: false,
        ..create_base_app()
    };

    let params = serde_json::json!({
        "dir_open": "",
        "dir_closed": "",
        "fallback": "",
        "icons": {
            "RS": "\u{e7a8}",
            "Py": "\u{e73c}"
        }
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    assert!(app.icons_enabled, "set_icon_map must enable icons");
    assert_eq!(
        app.icon_map.get("rs"),
        Some(&"\u{e7a8}".to_string()),
        "key must be stored lowercase"
    );
    assert_eq!(
        app.icon_map.get("py"),
        Some(&"\u{e73c}".to_string()),
        "mixed-case key must be lowered"
    );
}

#[test]
fn set_icon_map_missing_fields_ignored() {
    let mut app = App {
        icon_map: std::collections::HashMap::new(),
        icon_dir_open: "old_open".to_string(),
        icon_dir_closed: "old_closed".to_string(),
        icon_fallback: "old_fallback".to_string(),
        icons_enabled: false,
        ..create_base_app()
    };

    let params = serde_json::json!({
        "icons": {
            "rs": "\u{e7a8}"
        }
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    assert!(app.icons_enabled, "set_icon_map with icons must enable");
    // dir_* and fallback should remain unchanged since they weren't in the payload
    assert_eq!(app.icon_dir_open, "old_open");
    assert_eq!(app.icon_dir_closed, "old_closed");
    assert_eq!(app.icon_fallback, "old_fallback");
    assert_eq!(app.icon_map.get("rs"), Some(&"\u{e7a8}".to_string()));
}

#[test]
fn set_icon_map_empty_icons_does_not_enable() {
    let mut app = App {
        icon_map: std::collections::HashMap::new(),
        icons_enabled: false,
        ..create_base_app()
    };

    let params = serde_json::json!({
        "dir_open": "\u{f07c}",
        "dir_closed": "\u{f07b}",
        "fallback": "\u{f15b}",
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    assert!(!app.icons_enabled, "no icons key must not enable icons");
    assert_eq!(app.icon_dir_open, "\u{f07c}");
    assert_eq!(app.icon_dir_closed, "\u{f07b}");
    assert_eq!(app.icon_fallback, "\u{f15b}");
}

#[test]
fn set_icon_map_partial_icons_does_not_clear_existing() {
    let mut app = App {
        icon_map: {
            let mut m = std::collections::HashMap::new();
            m.insert("rs".to_string(), "\u{e7a8}".to_string());
            m
        },
        icons_enabled: false,
        ..create_base_app()
    };

    let params = serde_json::json!({
        "dir_open": "",
        "dir_closed": "",
        "fallback": "",
        "icons": {
            "py": "\u{e73c}"
        }
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    assert!(app.icons_enabled, "set_icon_map must enable icons");
    // Existing "rs" entry must be preserved, "py" added
    assert_eq!(app.icon_map.get("rs"), Some(&"\u{e7a8}".to_string()));
    assert_eq!(app.icon_map.get("py"), Some(&"\u{e73c}".to_string()));
}

#[test]
fn set_icon_map_then_clear_on_disable_clears_state() {
    let mut app = App {
        icon_map: {
            let mut m = std::collections::HashMap::new();
            m.insert("rs".to_string(), "\u{e7a8}".to_string());
            m
        },
        icon_dir_open: "\u{f07c}".to_string(),
        icon_dir_closed: "\u{f07b}".to_string(),
        icon_fallback: "\u{f15b}".to_string(),
        icons_enabled: true,
        ..create_base_app()
    };

    // Simulate the plugin being disabled: clearing icon state.
    if !app.icon_map.is_empty() {
        app.icons_enabled = false;
        app.icon_map.clear();
        app.icon_dir_open.clear();
        app.icon_dir_closed.clear();
        app.icon_fallback.clear();
    }

    assert!(
        !app.icons_enabled,
        "disabled plugin must clear icons_enabled"
    );
    assert!(app.icon_map.is_empty(), "icon_map must be cleared");
    assert!(app.icon_dir_open.is_empty());
    assert!(app.icon_dir_closed.is_empty());
    assert!(app.icon_fallback.is_empty());
}

// -- helpers ------------------------------------------------------------------

/// Minimal App for testing drain_plugin_actions in isolation.
#[test]
fn loader_set_extra_syntaxes_keeps_worker_serving() {
    use std::io::Write;
    let app = create_base_app();
    // Forward the current (empty) extra-syntax set to the worker; it must
    // rebuild its highlighter and keep answering file loads.
    app.loader_set_extra_syntaxes();
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    f.write_all(b"fn main() {}\n").unwrap();
    app.loader.request(LoadRequest::File {
        seq: 11,
        path: f.path().to_path_buf(),
    });
    let resp = app.loader.rx.recv().expect("worker response");
    assert!(matches!(resp, LoadResponse::File { seq: 11, .. }));
}

fn create_base_app() -> App {
    use crate::config::Config;
    use crate::highlight::Highlighter;
    use crate::plugin::PluginManager;
    use crate::theme::Theme;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;

    let theme = Theme::default();
    let highlighter = Highlighter::with_extra_syntaxes(&theme.syntax, &[]);

    App {
        root: PathBuf::from("/tmp"),
        nodes: Vec::new(),
        expanded: HashSet::new(),
        tree_selected: 0,
        tree_scroll: 0,
        tree_independent_scroll: false,
        content: Vec::new(),
        highlighted: Vec::new(),
        virtual_file: None,
        is_json: false,
        file_encoding: None,
        file_line_ending: None,
        show_pretty_json: false,
        json_pretty_text: Vec::new(),
        json_pretty_lines: Vec::new(),
        content_scroll: 0,
        content_hscroll: 0,
        active_line: 0,
        show_line_blame: false,
        word_wrap: false,
        current_file: None,
        current_syntax: None,
        is_diff: false,
        diff_side_by_side: false,
        viewing_revision: None,
        diff_rows: Vec::new(),
        content_title: None,
        focus: crate::app::Focus::Tree,
        search: None,
        last_search_query: String::new(),
        in_file_search: None,
        command_palette: None,
        history: None,
        theme_picker: None,
        plugin_picker: None,
        plugin_picker_area: ratatui::layout::Rect::default(),
        plugin_picker_offset: 0,
        recent_ring: Vec::new(),
        recent_files: None,
        recent_area: ratatui::layout::Rect::default(),
        recent_offset: 0,
        show_hidden: false,
        ignore_gitignore: false,
        tree_revision: 0,
        tree_width: 28,
        show_help: false,
        help_scroll: 0,
        help_tab: 0,
        help_area: ratatui::layout::Rect::default(),
        should_quit: false,
        theme,
        git_status_enabled: false,
        git_show_deleted: false,
        git_show_untracked: true,
        git_show_ignored: false,
        git_info: None,
        git_status_map: HashMap::new(),
        git_mode: false,
        git_mode_flat: false,
        show_scrollbar: false,
        show_scroll_percentage: false,
        show_line_numbers: false,
        show_blame: false,
        blame_col_width: 0,
        show_about: false,
        walk_errors: 0,
        config_error: None,
        auto_watch: false,
        show_file_info: false,
        indent_guides: false,
        icons_enabled: false,
        icon_map: HashMap::new(),
        icon_dir_open: String::new(),
        icon_dir_closed: String::new(),
        icon_fallback: String::new(),
        keys: crate::config::Keymap::default(),
        config: Config::default(),
        config_path: None,
        tree_area: ratatui::layout::Rect::default(),
        tree_offset: 0,
        tree_visible_indices: None,
        tree_guide_cache: None,
        content_area: ratatui::layout::Rect::default(),
        search_area: ratatui::layout::Rect::default(),
        search_offset: 0,
        command_palette_area: ratatui::layout::Rect::default(),
        command_palette_offset: 0,
        history_area: ratatui::layout::Rect::default(),
        history_offset: 0,
        theme_area: ratatui::layout::Rect::default(),
        theme_offset: 0,
        splitter_area: ratatui::layout::Rect::default(),
        last_click: None,
        last_breadcrumb_click: None,
        content_scrolled_at: std::time::Instant::now() - std::time::Duration::from_secs(10),
        highlighter,
        extra_syntaxes: Vec::new(),
        last_refresh: std::time::Instant::now(),
        file_watcher: None,
        file_watch_rx: None,
        file_watch_path: None,
        root_watcher: None,
        root_watch_rx: None,
        config_watcher: None,
        config_watch_rx: None,
        config_dirty: false,
        config_dirty_at: None,
        tree_dirty: false,
        tree_dirty_at: None,
        selection: None,
        drag_start: None,
        scrollbar_drag: false,
        splitter_drag: false,
        needs_clear: false,
        fold_regions: Vec::new(),
        folded: HashSet::new(),
        plugin_fold_regions: HashMap::new(),
        fold_display_map: Vec::new(),
        fold_gutter_rows: Vec::new(),
        yaml_error: None,
        yaml_anchor_count: 0,
        yaml_alias_count: 0,
        loader: crate::app::loader::Loader::new(&Theme::default(), Vec::new(), usize::MAX),
        load_seq: 0,
        loading: false,
        git_seq: 0,
        plugin_manager: PluginManager::new(Vec::new()),
        plugin_is_opening_file: false,
        plugin_message: None,
        plugin_error: None,
        pending_keypress: None,
        pending_keypress_handled: false,
        plugin_contributions: HashMap::new(),
        plugin_content: HashMap::new(),
        plugin_content_text: HashMap::new(),
        cursor_positions: HashMap::new(),
        plugin_content_active: false,
        plugin_content_active_path: None,
        status_message: None,
        breadcrumb_areas: Vec::new(),
        content_highlight_cache: RefCell::new(None),
        session_dirty: false,
        session_dirty_at: None,
        session_last_save: std::time::Instant::now(),
        clipboard_capture: Vec::new(),
        command_usage: crate::command_usage::UsageStats::default(),
        diff_mode: crate::app::DiffMode::default(),
        goto_line: None,
        tree_filter: None,
        new_version_available: None,
        update_rx: None,
    }
}

// -- set_content tests --------------------------------------------------------

#[test]
fn set_content_stores_spans_and_text_for_path() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.drain_plugin_actions_for_test(
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
fn set_content_for_current_file_resets_scroll_and_marks_active() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.current_file = Some(path.clone());
    app.content_scroll = 7;
    app.content_hscroll = 3;
    app.plugin_content_active = false;
    app.drain_plugin_actions_for_test(
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
    let mut app = create_base_app();
    app.current_file = Some(std::path::PathBuf::from("/tmp/open.md"));
    app.content_scroll = 7;
    app.content_hscroll = 3;
    app.plugin_content_active = false;
    app.drain_plugin_actions_for_test(
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
    let mut app = create_base_app();
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
    app.drain_plugin_actions_for_test(
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
    app.drain_plugin_actions_for_test(
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
    let mut app = create_base_app();
    let first = std::path::PathBuf::from("/tmp/first.md");
    let second = std::path::PathBuf::from("/tmp/second.md");
    app.current_file = Some(first.clone());
    app.content_scroll = 7;
    app.plugin_content_active = false;
    // Render first file — resets scroll
    app.drain_plugin_actions_for_test(
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
    app.drain_plugin_actions_for_test(
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
    let mut app = create_base_app();
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
    app.drain_plugin_actions_for_test(
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": many_lines}),
    );
    app.content_scroll = 20;
    // Simulate a same-file reload (apply_file_load preserves
    // plugin_content_active_path since same-file reload doesn't clear it)
    // The path remains current.
    let many_lines_b: Vec<String> = (0..30).map(|i| format!("other{i}")).collect();
    app.drain_plugin_actions_for_test(
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": many_lines_b}),
    );
    assert_eq!(
        app.content_scroll, 20,
        "re-render preserves scroll after same-file reload"
    );
}

// -- language provider protocol tests -----------------------------------------

#[test]
fn register_language_provider_stores_registration() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "my-lang-plugin",
        "register_language_provider",
        serde_json::json!({
            "extensions": ["rs", "rlib"],
            "capabilities": ["highlight", "fold"]
        }),
    );
    let cap = crate::plugin::Capability::Fold;
    assert!(
        app.plugin_manager.provider_for("rs", &cap).is_some(),
        "provider must be stored for 'rs' extension with Fold capability"
    );
    assert!(
        app.plugin_manager.provider_for("rlib", &cap).is_some(),
        "provider must be stored for 'rlib' extension"
    );
    assert!(
        app.plugin_manager.provider_for("py", &cap).is_none(),
        "unregistered extension must return None"
    );
}

#[test]
fn register_language_provider_overwrites_prior() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "my-lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["rs"], "capabilities": ["fold"]}),
    );
    // Re-register with different extensions.
    app.drain_plugin_actions_for_test(
        "my-lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["py"], "capabilities": ["fold"]}),
    );
    let cap = crate::plugin::Capability::Fold;
    assert!(
        app.plugin_manager.provider_for("py", &cap).is_some(),
        "new extension must be registered"
    );
    assert!(
        app.plugin_manager.provider_for("rs", &cap).is_none(),
        "old extension must no longer be registered after re-registration"
    );
}

#[test]
fn register_language_provider_parses_priority_and_defaults_to_zero() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "low",
        "register_language_provider",
        serde_json::json!({"extensions": ["rs"], "capabilities": ["fold"]}),
    );
    app.drain_plugin_actions_for_test(
        "high",
        "register_language_provider",
        serde_json::json!({"extensions": ["rs"], "capabilities": ["fold"], "priority": 10}),
    );
    let cap = crate::plugin::Capability::Fold;
    let winner = app
        .plugin_manager
        .provider_for("rs", &cap)
        .expect("a provider must be found");
    assert_eq!(
        winner.plugin_name, "high",
        "explicit higher priority must win over the default-0 registration"
    );
}

#[test]
fn register_language_provider_conflict_sets_plugin_message() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "first",
        "register_language_provider",
        serde_json::json!({"extensions": ["rs"], "capabilities": ["fold"]}),
    );
    assert!(app.plugin_message.is_none());
    app.drain_plugin_actions_for_test(
        "second",
        "register_language_provider",
        serde_json::json!({"extensions": ["rs"], "capabilities": ["fold"]}),
    );
    let msg = app
        .plugin_message
        .as_ref()
        .expect("a conflicting registration must set a status-bar warning");
    assert!(msg.contains("first"));
    assert!(msg.contains("second"));
}

// -- protocol 3: key_handled / plugin_error actions ---------------------------

#[test]
fn key_handled_true_sets_pending_keypress_handled() {
    let mut app = create_base_app();
    app.pending_keypress = Some(crate::app::PendingKeypress {
        key: crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::empty(),
        ),
        deadline: app.now() + Duration::from_secs(60),
    });
    app.pending_keypress_handled = false;
    app.drain_plugin_actions_for_test(
        "kp-plugin",
        "key_handled",
        serde_json::json!({"handled": true}),
    );
    assert!(app.pending_keypress_handled);
}

#[test]
fn key_handled_false_does_not_set_pending_keypress_handled() {
    let mut app = create_base_app();
    app.pending_keypress_handled = false;
    app.drain_plugin_actions_for_test(
        "kp-plugin",
        "key_handled",
        serde_json::json!({"handled": false}),
    );
    assert!(!app.pending_keypress_handled);
}

#[test]
fn key_handled_missing_field_does_not_set_pending_keypress_handled() {
    let mut app = create_base_app();
    app.pending_keypress_handled = false;
    app.drain_plugin_actions_for_test("kp-plugin", "key_handled", serde_json::json!({}));
    assert!(!app.pending_keypress_handled);
}

#[test]
fn stray_key_handled_reply_with_no_pending_keypress_is_ignored() {
    // A late `key_handled` reply for a keypress that already fell through
    // via its deadline (no keypress currently pending) must not be latched,
    // or it would incorrectly swallow whatever keypress gets deferred next.
    let mut app = create_base_app();
    app.pending_keypress = None;
    app.pending_keypress_handled = false;
    app.drain_plugin_actions_for_test(
        "kp-plugin",
        "key_handled",
        serde_json::json!({"handled": true}),
    );
    assert!(
        !app.pending_keypress_handled,
        "a stray reply with nothing pending must not set the handled flag"
    );
}

#[test]
fn process_pending_keypress_swallows_key_when_handled() {
    let mut app = create_base_app();
    app.tree_selected = 0;
    app.pending_keypress = Some(crate::app::PendingKeypress {
        key: crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::empty(),
        ),
        deadline: app.now() + Duration::from_secs(60),
    });
    app.pending_keypress_handled = true;

    app.process_pending_keypress();

    assert!(
        app.pending_keypress.is_none(),
        "a handled keypress must be cleared"
    );
    assert!(!app.pending_keypress_handled, "the handled flag must reset");
    assert_eq!(
        app.tree_selected, 0,
        "a swallowed key must not run normal-mode handling"
    );
}

#[test]
fn process_pending_keypress_falls_through_after_deadline() {
    let mut app = create_base_app();
    app.nodes = vec![
        crate::tree::TreeNode {
            path: "/tmp/a".into(),
            name: "a".into(),
            depth: 0,
            is_dir: false,
            deleted: false,
        },
        crate::tree::TreeNode {
            path: "/tmp/b".into(),
            name: "b".into(),
            depth: 0,
            is_dir: false,
            deleted: false,
        },
    ];
    app.tree_selected = 0;
    app.pending_keypress = Some(crate::app::PendingKeypress {
        key: crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::empty(),
        ),
        deadline: Instant::now() - Duration::from_millis(1),
    });
    app.pending_keypress_handled = false;

    app.process_pending_keypress();

    assert!(
        app.pending_keypress.is_none(),
        "an expired pending keypress must be cleared"
    );
    assert_eq!(
        app.tree_selected, 1,
        "a keypress past its deadline must fall through to normal-mode handling"
    );
}

#[test]
fn process_pending_keypress_noop_when_none_pending() {
    let mut app = create_base_app();
    app.pending_keypress = None;
    app.pending_keypress_handled = false;
    // Must not panic when there's nothing pending.
    app.process_pending_keypress();
    assert!(app.pending_keypress.is_none());
}

#[test]
fn preempt_pending_keypress_falls_through_immediately() {
    let mut app = create_base_app();
    app.nodes = vec![
        crate::tree::TreeNode {
            path: "/tmp/a".into(),
            name: "a".into(),
            depth: 0,
            is_dir: false,
            deleted: false,
        },
        crate::tree::TreeNode {
            path: "/tmp/b".into(),
            name: "b".into(),
            depth: 0,
            is_dir: false,
            deleted: false,
        },
    ];
    app.tree_selected = 0;
    app.pending_keypress = Some(crate::app::PendingKeypress {
        key: crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::empty(),
        ),
        // Deadline far in the future: only an explicit preempt resolves this.
        deadline: app.now() + Duration::from_secs(60),
    });

    app.preempt_pending_keypress();

    assert!(app.pending_keypress.is_none());
    assert_eq!(
        app.tree_selected, 1,
        "preempting must run normal-mode handling immediately, not wait for the deadline"
    );
}

#[test]
fn plugin_error_action_sets_plugin_error_distinct_from_plugin_message() {
    let mut app = create_base_app();
    app.plugin_message = Some("routine message".into());
    app.drain_plugin_actions_for_test(
        "noisy-plugin",
        "plugin_error",
        serde_json::json!({"message": "failed to parse file", "context": "on_file_open"}),
    );
    let err = app
        .plugin_error
        .as_ref()
        .expect("plugin_error action must set app.plugin_error");
    assert!(err.contains("noisy-plugin"));
    assert!(err.contains("failed to parse file"));
    assert!(err.contains("on_file_open"));
    assert_eq!(
        app.plugin_message.as_deref(),
        Some("routine message"),
        "plugin_error must not be routed through the routine show_message field"
    );
}

#[test]
fn plugin_error_action_without_context_still_sets_message() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "noisy-plugin",
        "plugin_error",
        serde_json::json!({"message": "boom"}),
    );
    let err = app.plugin_error.as_ref().expect("must be set");
    assert!(err.contains("boom"));
    assert!(err.contains("noisy-plugin"));
}

#[test]
fn set_fold_regions_applies_to_current_file() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/some/file.py");
    app.current_file = Some(path.clone());
    // Provide 3 lines of content so the display map has something to work with.
    app.content = vec!["a".into(), "  b".into(), "  c".into()];

    // A provider must declare the py/fold capability before regions are honored.
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["py"], "capabilities": ["fold"]}),
    );
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "set_fold_regions",
        serde_json::json!({
            "path": "/some/file.py",
            "regions": [[0, 2]]
        }),
    );

    assert_eq!(
        app.fold_regions.len(),
        1,
        "fold_regions must be updated for the current file"
    );
    assert_eq!(app.fold_regions[0].start, 0);
    assert_eq!(app.fold_regions[0].end, 2);
}

#[test]
fn set_fold_regions_stores_for_future_open() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/other/file.py");
    // current_file is None — the file is not yet open.
    app.current_file = None;

    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["py"], "capabilities": ["fold"]}),
    );
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "set_fold_regions",
        serde_json::json!({
            "path": "/other/file.py",
            "regions": [[1, 5], [10, 20]]
        }),
    );

    let stored = app.plugin_fold_regions.get(&path);
    assert!(stored.is_some(), "regions must be cached for future open");
    assert_eq!(stored.unwrap().len(), 2);
    // fold_regions on App should be untouched (no current file match).
    assert!(app.fold_regions.is_empty());
}

#[test]
fn set_fold_regions_ignored_without_registered_provider() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/some/file.py");
    app.current_file = Some(path.clone());
    app.content = vec!["a".into(), "  b".into(), "  c".into()];

    // No register_language_provider sent — the gate must reject the regions.
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "set_fold_regions",
        serde_json::json!({
            "path": "/some/file.py",
            "regions": [[0, 2]]
        }),
    );

    assert!(
        !app.plugin_fold_regions.contains_key(&path),
        "regions from an unregistered provider must not be cached"
    );
    assert!(
        app.fold_regions.is_empty(),
        "unregistered provider must not affect fold_regions"
    );
}

// -- plugin contribution tracking tests ---------------------------------------

#[test]
fn set_content_stamps_contribution() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.drain_plugin_actions_for_test(
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

#[test]
fn set_icon_map_stamps_contribution() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "iconize",
        "set_icon_map",
        serde_json::json!({"icons": {"rs": "\u{e7a8}"}, "dir_open": "", "dir_closed": "", "fallback": ""}),
    );
    let contrib = app.plugin_contributions.get("iconize").unwrap();
    assert!(contrib.has_icon_map, "has_icon_map must be true");
}

#[test]
fn set_fold_regions_stamps_contribution() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["py"], "capabilities": ["fold"]}),
    );
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "set_fold_regions",
        serde_json::json!({"path": "/tmp/doc.py", "regions": [[0, 2]]}),
    );
    let contrib = app.plugin_contributions.get("lang-plugin").unwrap();
    assert!(
        contrib
            .fold_region_paths
            .contains(&std::path::PathBuf::from("/tmp/doc.py")),
        "fold_region_paths must track the path"
    );
}

// -- teardown_plugin_contributions tests ---------------------------------------

#[test]
fn teardown_clears_content_state() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.current_file = Some(path.clone());
    app.drain_plugin_actions_for_test(
        "md-plugin",
        "set_content",
        serde_json::json!({"path": "/tmp/doc.md", "lines": ["hello", "world"]}),
    );
    assert!(
        app.plugin_content_active,
        "content must be active for current file"
    );
    assert!(app.plugin_content.contains_key(&path));

    app.teardown_plugin_contributions("md-plugin");

    assert!(
        !app.plugin_content.contains_key(&path),
        "content must be removed"
    );
    assert!(
        !app.plugin_content_text.contains_key(&path),
        "text must be removed"
    );
    assert!(!app.plugin_content_active, "content active must be cleared");
    assert!(
        app.plugin_contributions.is_empty(),
        "contributions entry must be removed"
    );
}

#[test]
fn teardown_clears_icon_state() {
    let mut app = App {
        icons_enabled: true,
        icon_map: {
            let mut m = std::collections::HashMap::new();
            m.insert("rs".to_string(), "\u{e7a8}".to_string());
            m
        },
        icon_dir_open: "\u{f07c}".to_string(),
        icon_dir_closed: "\u{f07b}".to_string(),
        icon_fallback: "\u{f15b}".to_string(),
        plugin_contributions: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "iconize".to_string(),
                crate::plugin::PluginContributions {
                    has_icon_map: true,
                    ..Default::default()
                },
            );
            m
        },
        ..create_base_app()
    };

    app.teardown_plugin_contributions("iconize");

    assert!(!app.icons_enabled, "icons_enabled must be cleared");
    assert!(app.icon_map.is_empty(), "icon_map must be cleared");
    assert!(app.icon_dir_open.is_empty());
    assert!(app.icon_dir_closed.is_empty());
    assert!(app.icon_fallback.is_empty());
}

#[test]
fn teardown_clears_fold_regions() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.py");
    app.current_file = Some(path.clone());
    app.plugin_fold_regions.insert(
        path.clone(),
        vec![crate::fold::FoldRegion { start: 0, end: 2 }],
    );
    app.fold_regions = vec![crate::fold::FoldRegion { start: 0, end: 2 }];
    app.plugin_contributions.insert(
        "lang-plugin".to_string(),
        crate::plugin::PluginContributions {
            fold_region_paths: {
                let mut s = std::collections::HashSet::new();
                s.insert(path.clone());
                s
            },
            ..Default::default()
        },
    );

    app.teardown_plugin_contributions("lang-plugin");

    assert!(
        !app.plugin_fold_regions.contains_key(&path),
        "fold regions must be removed"
    );
    assert!(
        app.fold_regions.is_empty(),
        "active fold state must be cleared"
    );
}

#[test]
fn teardown_removes_provider_registrations() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "register_language_provider",
        serde_json::json!({"extensions": ["py"], "capabilities": ["fold"]}),
    );
    // Also stamp the contribution so teardown finds something.
    app.drain_plugin_actions_for_test(
        "lang-plugin",
        "set_fold_regions",
        serde_json::json!({"path": "/tmp/doc.py", "regions": [[0, 2]]}),
    );
    assert!(
        app.plugin_manager
            .provider_for("py", &crate::plugin::Capability::Fold)
            .is_some(),
        "provider must be registered before teardown"
    );

    app.teardown_plugin_contributions("lang-plugin");

    assert!(
        app.plugin_manager
            .provider_for("py", &crate::plugin::Capability::Fold)
            .is_none(),
        "provider registration must be removed"
    );
}

#[test]
fn teardown_noop_for_unknown_plugin() {
    let mut app = create_base_app();
    app.plugin_contributions.clear();
    // Must not panic.
    app.teardown_plugin_contributions("nonexistent");
}

// -- status message TTL tests --------------------------------------------------

#[test]
fn status_message_expires_after_ttl() {
    let mut app = create_base_app();
    // Set a status message with a back-dated timestamp so it's expired.
    app.status_message = Some(StatusMessage {
        text: "old message".into(),
        set_at: Instant::now() - Duration::from_secs(10),
    });
    app.tick();
    assert!(
        app.status_message.is_none(),
        "expired status message must be cleared on tick"
    );
}

#[test]
fn status_message_survives_fresh_tick() {
    let mut app = create_base_app();
    app.status_message = Some(StatusMessage {
        text: "recent message".into(),
        set_at: Instant::now(),
    });
    app.tick();
    assert!(
        app.status_message.is_some(),
        "fresh status message must survive a tick"
    );
    assert_eq!(
        app.status_message.as_ref().unwrap().text,
        "recent message",
        "text must be preserved"
    );
}

#[test]
fn set_status_creates_message_with_timestamp() {
    let mut app = create_base_app();
    app.set_status("test message");
    let sm = app.status_message.expect("message must be set");
    assert_eq!(sm.text, "test message");
    // The timestamp should be recent (within the last second).
    assert!(
        sm.set_at.elapsed() < Duration::from_secs(1),
        "timestamp must be recent"
    );
}

// -- git status refresh tests -------------------------------------------------

#[test]
fn apply_git_status_load_updates_map_and_info() {
    let mut app = create_base_app();
    assert!(app.git_status_map.is_empty());
    assert!(app.git_info.is_none());

    let mut sm = std::collections::HashMap::new();
    sm.insert(
        std::path::PathBuf::from("/repo/file.txt"),
        crate::git::GitStatus::Modified,
    );
    let info = crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".into()),
        ahead: 0,
        behind: 0,
        total_changed: 1,
        staged: 0,
        untracked: 0,
    };
    let load = GitStatusLoad {
        status_map: sm.clone(),
        info: Some(info.clone()),
    };
    app.apply_git_status_load(load);

    assert_eq!(
        app.git_status_map
            .get(&std::path::PathBuf::from("/repo/file.txt")),
        Some(&crate::git::GitStatus::Modified)
    );
    assert_eq!(
        app.git_info.as_ref().map(|i| &i.head),
        Some(&crate::git::GitHead::Branch("main".into()))
    );
}

#[test]
fn apply_git_status_load_rebuilds_tree_in_git_mode() {
    use std::io::Write;
    let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    f.write_all(b"fn main() {}\n").unwrap();
    let dir = f.path().parent().unwrap().to_path_buf();

    let mut app = App {
        root: dir.clone(),
        git_mode: true,
        git_status_enabled: true,
        git_seq: 0,
        ..create_base_app()
    };
    // Build initial tree (empty status → all nodes shown in non-git mode).
    // In git mode an empty map yields an empty filtered tree.
    app.nodes = vec![crate::tree::TreeNode {
        path: f.path().to_path_buf(),
        name: "test.rs".into(),
        depth: 0,
        is_dir: false,
        deleted: false,
    }];
    let load = GitStatusLoad {
        status_map: std::collections::HashMap::new(),
        info: None,
    };
    app.apply_git_status_load(load);
    // Map is still empty, so git-mode filter removes the node.
    assert!(
        app.nodes.is_empty(),
        "git mode with empty map must yield empty tree"
    );
}

#[test]
fn request_git_status_refresh_enqueues_and_applies() {
    let mut app = create_base_app();
    assert!(app.git_status_map.is_empty());
    assert!(app.git_info.is_none());

    app.request_git_status_refresh();
    app.pump_loads();

    // For a non-git-repo root the map/info remain empty — the important
    // thing is the pipeline doesn't crash and the fields are accessible.
    assert!(app.git_status_map.is_empty());
    assert!(app.git_info.is_none());
}

#[test]
fn request_git_status_refresh_ignore_gitignore_includes_ignored() {
    // When ignore_gitignore is true, request_git_status_refresh must pass
    // include_ignored=true to repo_status even when git_show_ignored is false.
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    fs::write(root.join("tracked.txt"), "hello\n").unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    fs::write(root.join("build.log"), "log\n").unwrap();

    let mut app = create_base_app();
    app.root = root.canonicalize().unwrap();
    app.git_status_enabled = true;
    app.ignore_gitignore = true;
    // git_show_ignored is false from create_base_app.
    assert!(!app.git_show_ignored);
    // The map must be empty before the refresh.
    assert!(app.git_status_map.is_empty());

    app.request_git_status_refresh();
    app.pump_loads();

    let ignored = app.root.join("build.log");
    assert_eq!(
        app.git_status_map.get(&ignored),
        Some(&crate::git::GitStatus::Ignored),
        "request_git_status_refresh with ignore_gitignore=true must include ignored files"
    );
}

// -- Supersession (stale worker responses discarded) --------------------------

#[test]
fn request_open_file_supersession_discards_stale_response() {
    use std::io::Write;
    let mut app = create_base_app();
    let mut a = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    a.write_all(b"AAA\n").unwrap();
    let mut b = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
    b.write_all(b"BBB\n").unwrap();

    // Two requests fired back-to-back before either is drained: only the
    // second (latest load_seq) result should ever be applied.
    app.request_open_file(a.path());
    app.request_open_file(b.path());
    app.pump_loads();

    assert_eq!(
        app.current_file,
        Some(b.path().to_path_buf()),
        "only the newest request_open_file must be applied"
    );
    // Plain files load lazily via VirtualFile; check its raw bytes rather
    // than `content` (which stays empty on this path).
    let vf = app.virtual_file.as_ref().expect("virtual_file must be set");
    assert_eq!(vf.raw_bytes(), b"BBB\n");
}

#[test]
fn request_working_tree_diff_supersession_discards_stale_response() {
    use std::process::Command;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    git(&["init", "-q"]);
    fs::write(root.join("a.txt"), "a\n").unwrap();
    fs::write(root.join("b.txt"), "b\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    fs::write(root.join("a.txt"), "a changed\n").unwrap();
    fs::write(root.join("b.txt"), "b changed\n").unwrap();

    let mut app = create_base_app();
    app.root = root.canonicalize().unwrap();

    // Two requests fired back-to-back before either is drained: only the
    // second (latest load_seq) result should ever be applied.
    app.request_working_tree_diff(&root.join("a.txt"));
    app.request_working_tree_diff(&root.join("b.txt"));
    app.pump_loads();

    assert_eq!(
        app.current_file,
        Some(root.join("b.txt")),
        "only the newest request_working_tree_diff must be applied"
    );
}

#[test]
fn apply_response_ignores_stale_file_seq() {
    let mut app = create_base_app();
    app.load_seq = 5;
    app.current_file = None;

    // Contents don't matter: the response is intentionally stale and must
    // never be applied.
    let stale = tempfile::NamedTempFile::new().unwrap();
    let applied = app.apply_response(LoadResponse::File {
        seq: 4,
        path: stale.path().to_path_buf(),
        load: Box::new(compute_file_load(
            stale.path(),
            &app.highlighter,
            usize::MAX,
        )),
    });

    assert!(!applied, "stale seq must not be reported as applied");
    assert!(
        app.current_file.is_none(),
        "stale response must not update current_file"
    );
}

// -- Extension trait ----------------------------------------------------------

/// Extension trait to drive the production `handle_plugin_action` with a
/// synthetic action, so tests exercise the real code path instead of a copy.
trait DrainPluginActionsForTest {
    fn drain_plugin_actions_for_test(
        &mut self,
        _name: &str,
        _action: &str,
        _params: serde_json::Value,
    );
}

impl DrainPluginActionsForTest for App {
    fn drain_plugin_actions_for_test(
        &mut self,
        name: &str,
        action: &str,
        params: serde_json::Value,
    ) {
        self.handle_plugin_action(name, action, &params);
    }
}

// -- drain_plugin_actions crash-message tests --------------------------------

/// Shared crate-wide lock serialising every test that sets `MANTIS_STATE_DIR`
/// (a process-global env var) — see `crate::session::STATE_DIR_ENV_LOCK`.
#[cfg(unix)]
use crate::session::STATE_DIR_ENV_LOCK;

#[test]
#[cfg(unix)]
fn drain_plugin_actions_surfaces_crash_diagnostics_in_plugin_message() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;

    let _lock = STATE_DIR_ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let dir = std::env::temp_dir().join(format!("tv_refresh_crash_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\necho 'panic: oh no' >&2\nexit 1\n").unwrap();
    drop(f);
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let entry = crate::plugin::PluginEntry {
        path: script.clone(),
        enabled: false,
        ..Default::default()
    };
    let mut app = App {
        plugin_manager: crate::plugin::PluginManager::new(vec![("crashy".to_string(), entry)]),
        ..create_base_app()
    };
    app.plugin_manager
        .activate_one("crashy", None)
        .expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        app.drain_plugin_actions();
        if app.plugin_message.is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "plugin was never seen as dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let message = app
        .plugin_message
        .as_ref()
        .expect("crash must set a plugin_message");
    assert!(
        message.contains("panic: oh no"),
        "message must surface the last stderr line, got: {message:?}"
    );
    assert!(
        message.contains("full log:"),
        "message must point to the on-disk log, got: {message:?}"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn test_handle_config_change_reloads_safe_settings() {
    let mut app = create_base_app();

    // Create a temporary mantis.toml config file
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("mantis.toml");

    // Write a new config with safe settings changed
    let toml_content = r#"
[tree]
show_hidden = true
width = 42
indent_guides = true
icons = true

[content]
word_wrap = true
line_numbers = false
scrollbar = false
scroll_percentage = true
watch = true
show_file_info = true
"#;
    std::fs::write(&config_path, toml_content).expect("write config");

    app.config_path = Some(config_path);
    app.handle_config_change();

    // Verify that safe settings are hot-reloaded
    assert!(app.show_hidden);
    assert_eq!(app.tree_width, 42);
    assert!(app.word_wrap);
    assert!(!app.show_line_numbers);
    assert!(!app.show_scrollbar);
    assert!(app.auto_watch);

    // Status message should indicate config is reloaded
    let status = app.status_message.as_ref().expect("status message");
    assert!(
        status.text.contains("reloaded"),
        "got status: {:?}",
        status.text
    );
}

#[test]
fn test_handle_config_change_warns_on_plugins_changed() {
    let mut app = create_base_app();

    // Let's set some default plugins first
    use crate::plugin::PluginEntry;
    use std::collections::HashMap;
    let mut plugins = HashMap::new();
    plugins.insert(
        "test_plugin".to_string(),
        PluginEntry {
            path: std::path::PathBuf::from("/bin/true"),
            enabled: true,
            ..Default::default()
        },
    );
    app.config.plugins = plugins;

    // Create a temporary mantis.toml config file
    let dir = tempfile::tempdir().expect("tempdir");
    let config_path = dir.path().join("mantis.toml");

    // Write a config with plugins changed
    let toml_content = r#"
[plugins.test_plugin]
path = "/bin/false"
enabled = true
"#;
    std::fs::write(&config_path, toml_content).expect("write config");

    app.config_path = Some(config_path);
    app.handle_config_change();

    // Status message should indicate that a restart is required
    let status = app.status_message.as_ref().expect("status message");
    assert!(
        status.text.contains("restart to apply"),
        "got status: {:?}",
        status.text
    );
}
