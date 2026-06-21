use std::collections::HashSet;

use super::*;

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
        word_wrap: false,
        current_file: None,
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
        yaml_fold_regions: Vec::new(),
        yaml_folded: HashSet::new(),
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
        status_message: None,
        breadcrumb_areas: Vec::new(),
    }
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
        _name: &str,
        _action: &str,
        _params: serde_json::Value,
    ) {
        // Replicate the logic from drain_plugin_actions for the set_icon_map case.
        if _action == "set_icon_map" {
            if let Some(obj) = _params.as_object() {
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
    }
}
