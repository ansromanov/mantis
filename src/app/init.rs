//! App construction: `App::new` and its helpers.
//!
//! Building the app requires walking the root directory, loading git status,
//! resolving the theme, seeding bundled plugins, discovering plugin manifests,
//! constructing the highlighter and loader, spawning the plugin subprocess
//! manager, and applying any persisted session state on top of the config.
//! This module isolates the constructor so the main `App` struct definition
//! in `mod.rs` stays focused on the data model.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::highlight::Highlighter;
use crate::plugin::{self, PluginManager};
use crate::tree::build_visible;

use super::loader::Loader;
use super::{App, Focus};

impl App {
    /// Builds the app: walks the root directory, loads git status, resolves
    /// the theme, and opens the first selected file.
    pub fn new(
        root: PathBuf,
        mut cfg: crate::config::Config,
        config_path: Option<std::path::PathBuf>,
        config_error: Option<String>,
    ) -> anyhow::Result<Self> {
        let expanded = HashSet::new();
        let git_status_enabled = cfg.git.status;
        let git_show_deleted = cfg.git.show_deleted;
        let git_show_untracked = cfg.git.show_untracked;
        let git_show_ignored = cfg.git.show_ignored;
        let effective_show_ignored = cfg.git.show_ignored || cfg.git.ignore_gitignore;
        let git_status_map = if git_status_enabled {
            crate::git::repo_status(&root, cfg.git.show_untracked, effective_show_ignored)
        } else {
            HashMap::new()
        };
        let git_info = if git_status_enabled {
            crate::git::repo_info(&root)
        } else {
            None
        };
        let deleted = super::deleted_set(&git_status_map, git_show_deleted);
        let (nodes, walk_errors) = build_visible(
            &root,
            &expanded,
            cfg.tree.show_hidden,
            cfg.git.ignore_gitignore,
            &deleted,
        );
        let theme = cfg.theme.resolve();

        // Seed bundled plugins into the config map (insert-if-absent) so the
        // plugin palette shows them even when mantis.toml has no [plugins] section.
        for (name, entry) in plugin::bundled_plugin_entries() {
            cfg.plugins.entry(name).or_insert(entry);
        }

        // Discover plugins from the plugin directory via plugin.toml manifests.
        // Explicit mantis.toml entries win on name collision; discovered plugins
        // default to disabled so no freshly fetched code runs without user opt-in.
        for (name, entry) in plugin::manifest::discover(&plugin::default_plugin_dir()) {
            cfg.plugins.entry(name).or_insert(entry);
        }

        let saved_config = cfg.clone();

        // Collect extra syntax definitions from plugins before constructing
        // the highlighter and loader (they need them at creation time).
        let mut plugin_entries: Vec<_> = cfg.plugins.clone().into_iter().collect();
        plugin_entries.sort_by(|a, b| a.0.cmp(&b.0));
        let extra_syntaxes = plugin::load_extra_syntaxes(&plugin_entries);

        let highlighter = Highlighter::with_extra_syntaxes(&theme.syntax, &extra_syntaxes);
        let loader = Loader::new(&theme, extra_syntaxes.clone());

        // Syntax plugins go to the highlighter AND the manager (so they appear
        // in the plugin palette). The manager skips non-Process entries in
        // activate_all/activate_one.
        let mut plugin_manager = PluginManager::new(plugin_entries);
        plugin_manager.activate_all(cfg.theme.name.as_deref(), &theme);
        let plugin_spawn_error = plugin_manager
            .take_spawn_errors()
            .into_iter()
            .next()
            .map(|e| format!("[plugin] {e}"));
        let mut app = App {
            root,
            nodes,
            expanded,
            tree_selected: 0,
            tree_scroll: 0,
            tree_independent_scroll: cfg.tree.independent_scroll,
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
            word_wrap: cfg.content.word_wrap,
            current_file: None,
            current_syntax: None,
            is_diff: false,
            diff_mode: cfg.git.diff.mode,
            diff_side_by_side: cfg.git.diff.side_by_side,
            viewing_revision: None,
            diff_rows: Vec::new(),
            content_title: None,
            focus: Focus::Tree,
            search: None,
            last_search_query: String::new(),
            in_file_search: None,
            tree_filter: None,
            goto_line: None,
            command_usage: crate::command_usage::UsageStats::load(),
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
            show_hidden: cfg.tree.show_hidden,
            ignore_gitignore: cfg.git.ignore_gitignore,
            tree_revision: 0,
            tree_width: cfg.tree.width,
            show_help: false,
            help_scroll: 0,
            should_quit: false,
            theme,
            git_status_enabled,
            git_show_deleted,
            git_show_untracked,
            git_show_ignored,
            git_info,
            git_status_map,
            git_mode: false,
            git_mode_flat: false,
            show_scrollbar: cfg.content.scrollbar,
            show_scroll_percentage: cfg.content.scroll_percentage,
            show_line_numbers: cfg.content.line_numbers,
            show_blame: false,
            blame_col_width: 0,
            show_about: false,
            walk_errors,
            config_error,
            auto_watch: cfg.content.watch,
            show_file_info: cfg.content.show_file_info,
            indent_guides: cfg.tree.indent_guides,
            icons_enabled: cfg.tree.icons,
            icon_map: HashMap::new(),
            icon_dir_open: String::new(),
            icon_dir_closed: String::new(),
            icon_fallback: String::new(),
            keys: cfg.keys,
            config: saved_config,
            config_path,
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
            content_scrolled_at: Instant::now() - std::time::Duration::from_secs(10),
            highlighter,
            extra_syntaxes,
            last_refresh: Instant::now(),
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
            loader,
            load_seq: 0,
            loading: false,
            git_seq: 0,
            plugin_manager,
            plugin_is_opening_file: false,
            plugin_message: plugin_spawn_error,
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
            session_last_save: Instant::now(),
        };

        // Load session state and apply it over the config-driven defaults.
        let session_state = crate::session::load(&app.root);
        let has_session_override = session_state.is_some();
        if let Some(ref s) = session_state {
            // Restore expanded directories that still exist.
            for dir in &s.expanded {
                if dir.starts_with(&app.root) && dir.is_dir() {
                    app.expanded.insert(dir.clone());
                }
            }
        }

        if has_session_override {
            app.rebuild(true);
        }

        // If the session specifies a file, select it in the tree.
        if let Some(ref s) = session_state {
            if let Some(ref cf) = s.current_file {
                if let Some(i) = app.nodes.iter().position(|n| n.path == *cf) {
                    app.tree_selected = i;
                }
            }
        }

        // Open the selected file synchronously so it is visible on the first
        // frame (and so callers/tests can observe content right after
        // construction).
        app.open_selected_sync();

        // Restore scroll/active-line position after the file is loaded.
        if let Some(ref s) = session_state {
            if s.current_file.as_deref() == app.current_file.as_deref() {
                app.set_content_scroll(s.content_scroll);
                app.active_line = s.active_line.min(app.line_count().saturating_sub(1));
            }
        }

        Ok(app)
    }
}

#[cfg(test)]
#[path = "init_test.rs"]
mod init_tests;
