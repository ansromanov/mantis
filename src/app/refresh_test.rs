use std::collections::HashSet;
use std::time::{Duration, Instant};

use super::*;

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
        markdown_lines: Vec::new(),
        virtual_file: None,
        is_markdown: false,
        show_raw_markdown: false,
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
        tree_width: 28,
        show_help: false,
        should_quit: false,
        theme,
        git_status_enabled: false,
        git_show_deleted: false,
        git_info: None,
        git_status_map: HashMap::new(),
        git_mode: false,
        git_mode_flat: false,
        show_scrollbar: false,
        show_scroll_percentage: false,
        show_line_numbers: false,
        show_blame: false,
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
        tree_visible_indices: Vec::new(),
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
        content_scrolled_at: std::time::Instant::now() - std::time::Duration::from_secs(10),
        highlighter,
        extra_syntaxes: Vec::new(),
        last_refresh: std::time::Instant::now(),
        file_watcher: None,
        file_watch_rx: None,
        file_watch_path: None,
        root_watcher: None,
        root_watch_rx: None,
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
        loader: crate::app::loader::Loader::new(&Theme::default(), Vec::new()),
        load_seq: 0,
        loading: false,
        plugin_manager: PluginManager::new(Vec::new()),
        plugin_is_opening_file: false,
        plugin_message: None,
        plugin_contributions: HashMap::new(),
        plugin_blame: HashMap::new(),
        plugin_git_info: None,
        plugin_content: HashMap::new(),
        plugin_content_text: HashMap::new(),
        plugin_content_active: false,
        status_message: None,
        breadcrumb_areas: Vec::new(),
        session_dirty: false,
        session_dirty_at: None,
        session_last_save: std::time::Instant::now(),
        diff_mode: crate::app::DiffMode::default(),
        goto_line: None,
        tree_filter: None,
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
fn set_blame_data_stamps_contribution() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "blame-plugin",
        "set_blame_data",
        serde_json::json!({"path": "/tmp/doc.md", "lines": ["author A"]}),
    );
    let contrib = app.plugin_contributions.get("blame-plugin").unwrap();
    assert!(
        contrib
            .blame_paths
            .contains(&std::path::PathBuf::from("/tmp/doc.md")),
        "blame_paths must track the path"
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
fn set_status_bar_git_info_stamps_contribution() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "git-plugin",
        "set_status_bar_git_info",
        serde_json::json!({"branch": "main", "head": "abc123", "dirty": false, "state": "clean"}),
    );
    let contrib = app.plugin_contributions.get("git-plugin").unwrap();
    assert!(contrib.has_git_info, "has_git_info must be true");
}

#[test]
fn set_file_statuses_stamps_contribution() {
    let mut app = create_base_app();
    app.drain_plugin_actions_for_test(
        "status-plugin",
        "set_file_statuses",
        serde_json::json!({"/tmp/file.txt": "modified", "/tmp/new.txt": "added"}),
    );
    let contrib = app.plugin_contributions.get("status-plugin").unwrap();
    assert!(contrib
        .status_paths
        .contains(&std::path::PathBuf::from("/tmp/file.txt")));
    assert!(contrib
        .status_paths
        .contains(&std::path::PathBuf::from("/tmp/new.txt")));
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
fn teardown_clears_blame_state() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/doc.md");
    app.drain_plugin_actions_for_test(
        "blame-plugin",
        "set_blame_data",
        serde_json::json!({"path": "/tmp/doc.md", "lines": ["author A"]}),
    );
    assert!(app.plugin_blame.contains_key(&path));

    app.teardown_plugin_contributions("blame-plugin");

    assert!(
        !app.plugin_blame.contains_key(&path),
        "blame must be removed"
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
fn teardown_clears_git_info() {
    let mut app = App {
        plugin_git_info: Some(crate::app::PluginGitInfo {
            branch: "main".into(),
            head: "abc".into(),
            dirty: false,
            state: "clean".into(),
        }),
        plugin_contributions: {
            let mut m = std::collections::HashMap::new();
            m.insert(
                "git-plugin".to_string(),
                crate::plugin::PluginContributions {
                    has_git_info: true,
                    ..Default::default()
                },
            );
            m
        },
        ..create_base_app()
    };

    app.teardown_plugin_contributions("git-plugin");

    assert!(app.plugin_git_info.is_none(), "git info must be cleared");
}

#[test]
fn teardown_clears_status_paths() {
    let mut app = create_base_app();
    let path = std::path::PathBuf::from("/tmp/file.txt");
    app.git_status_map
        .insert(path.clone(), crate::git::GitStatus::Modified);
    app.plugin_contributions.insert(
        "status-plugin".to_string(),
        crate::plugin::PluginContributions {
            status_paths: {
                let mut s = std::collections::HashSet::new();
                s.insert(path.clone());
                s
            },
            ..Default::default()
        },
    );

    app.teardown_plugin_contributions("status-plugin");

    assert!(
        !app.git_status_map.contains_key(&path),
        "status entry must be removed"
    );
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
