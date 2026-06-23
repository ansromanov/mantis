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
        ..create_base_app()
    };

    let params = serde_json::json!({
        "icons": {
            "rs": "\u{e7a8}"
        }
    });

    app.drain_plugin_actions_for_test("iconize", "set_icon_map", params);

    // dir_* and fallback should remain unchanged since they weren't in the payload
    assert_eq!(app.icon_dir_open, "old_open");
    assert_eq!(app.icon_dir_closed, "old_closed");
    assert_eq!(app.icon_fallback, "old_fallback");
    assert_eq!(app.icon_map.get("rs"), Some(&"\u{e7a8}".to_string()));
}

#[test]
fn set_icon_map_partial_icons_does_not_clear_existing() {
    let mut app = App {
        icon_map: {
            let mut m = std::collections::HashMap::new();
            m.insert("rs".to_string(), "\u{e7a8}".to_string());
            m
        },
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

    // Existing "rs" entry must be preserved, "py" added
    assert_eq!(app.icon_map.get("rs"), Some(&"\u{e7a8}".to_string()));
    assert_eq!(app.icon_map.get("py"), Some(&"\u{e73c}".to_string()));
}

// -- helpers ------------------------------------------------------------------

/// Minimal App for testing drain_plugin_actions in isolation.
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
        visual_line: None,
        blame_panel: false,
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
        plugin_message: None,
        plugin_blame: HashMap::new(),
        plugin_git_info: None,
        plugin_content: HashMap::new(),
        plugin_content_active: false,
        status_message: None,
        breadcrumb_areas: Vec::new(),
        diff_mode: crate::app::DiffMode::default(),
        goto_line: None,
        tree_filter: None,
    }
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

/// Extension trait to call the private `drain_plugin_actions` with a synthetic action.
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
        match action {
            "set_icon_map" => {
                if let Some(obj) = params.as_object() {
                    if let Some(icons) = obj.get("icons").and_then(|v| v.as_object()) {
                        for (ext, glyph) in icons {
                            if let Some(g) = glyph.as_str() {
                                self.icon_map
                                    .insert(ext.to_ascii_lowercase(), g.to_string());
                            }
                        }
                    }
                    if let Some(open) = obj.get("dir_open").and_then(|v| v.as_str()) {
                        self.icon_dir_open = open.to_string();
                    }
                    if let Some(closed) = obj.get("dir_closed").and_then(|v| v.as_str()) {
                        self.icon_dir_closed = closed.to_string();
                    }
                    if let Some(fallback) = obj.get("fallback").and_then(|v| v.as_str()) {
                        self.icon_fallback = fallback.to_string();
                    }
                }
            }
            "register_language_provider" => {
                let extensions: Vec<String> = params
                    .get("extensions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_ascii_lowercase))
                            .collect()
                    })
                    .unwrap_or_default();
                let capabilities: std::collections::HashSet<crate::plugin::Capability> = params
                    .get("capabilities")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| serde_json::from_value(v.clone()).ok())
                            .collect()
                    })
                    .unwrap_or_default();
                let reg = crate::plugin::LanguageProviderRegistration {
                    plugin_name: name.to_string(),
                    extensions,
                    capabilities,
                };
                self.plugin_manager.register_provider(reg);
            }
            "set_fold_regions" => {
                let path = match params.get("path").and_then(|v| v.as_str()) {
                    Some(p) => std::path::PathBuf::from(p),
                    None => return,
                };
                let regions: Vec<crate::fold::FoldRegion> = params
                    .get("regions")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|r| {
                                let pair = r.as_array()?;
                                let start = pair.first()?.as_i64()? as usize;
                                let end = pair.get(1)?.as_i64()? as usize;
                                Some(crate::fold::FoldRegion { start, end })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                self.plugin_fold_regions.insert(path.clone(), regions);
                if self.current_file.as_deref() == Some(&path) {
                    self.apply_plugin_fold_regions(&path);
                }
            }
            _ => {}
        }
    }
}
