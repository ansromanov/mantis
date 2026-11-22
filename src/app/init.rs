//! App construction: `App::new` and its helpers.
//!
//! Building the app requires walking the root directory, loading git status,
//! resolving the theme, constructing the highlighter and loader, and applying
//! any persisted session state on top of the config.
//! This module isolates the constructor so the main `App` struct definition
//! in `mod.rs` stays focused on the data model.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use crate::highlight::Highlighter;
use crate::tree::build_visible;

use super::loader::Loader;
use super::{App, Focus};

impl App {
    /// Builds the app: walks the root directory, loads git status, resolves
    /// the theme, and opens the first selected file.
    pub fn new(
        root: PathBuf,
        cfg: crate::config::Config,
        config_path: Option<std::path::PathBuf>,
        config_error: Option<String>,
    ) -> anyhow::Result<Self> {
        let expanded = HashSet::new();
        let git_status_enabled = cfg.git.status;
        let git_show_deleted = cfg.git.show_deleted;
        let git_show_untracked = cfg.git.show_untracked;
        let git_show_ignored = cfg.git.show_ignored;
        let effective_show_ignored = cfg.git.show_ignored || cfg.git.ignore_gitignore;
        let git_details = if git_status_enabled {
            crate::git::repo_status_details(&root, cfg.git.show_untracked, effective_show_ignored)
        } else {
            crate::git::GitStatusDetails::default()
        };
        let git_status_map = git_details.status_map;
        let git_staged_files = git_details.staged;
        let git_unstaged_files = git_details.unstaged;
        let git_untracked_files = git_details.untracked;
        let git_info = if git_status_enabled {
            crate::git::repo_info(&root)
        } else {
            None
        };
        let worktree_count = crate::git::worktree_list(&root).len();
        let deleted = super::deleted_set(&git_status_map, git_show_deleted);
        let (nodes, walk_errors) = build_visible(
            &root,
            &expanded,
            cfg.tree.show_hidden,
            cfg.git.ignore_gitignore,
            &deleted,
        );
        let theme = cfg.theme.resolve();

        let saved_config = cfg.clone();

        let highlighter = Highlighter::new(&theme.syntax);
        let loader = Loader::new(&theme, cfg.content.prettify_size_limit);
        let (diagnostics_tx, diagnostics_rx) = std::sync::mpsc::channel();
        let (new_version_available, update_rx) =
            crate::update::check_for_updates(cfg.updates.check);

        let mut app = App {
            initial_root: root.clone(),
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
            is_csv: false,
            file_encoding: None,
            file_line_ending: None,
            show_pretty_json: false,
            json_pretty_text: Vec::new(),
            json_pretty_lines: Vec::new(),
            json_path_map: Vec::new(),
            show_csv_table: false,
            csv_table_text: Vec::new(),
            csv_table_lines: Vec::new(),
            secret_original: Vec::new(),
            secret_masked: false,
            secret_revealed: false,
            content_scroll: 0,
            content_hscroll: 0,
            scroll_wrap_width: 0,
            active_line: 0,
            show_line_blame: false,
            word_wrap: cfg.content.word_wrap,
            current_file: None,
            current_syntax: None,
            is_diff: false,
            diff_mode: cfg.git.diff.mode,
            diff_side_by_side: cfg.git.diff.side_by_side,
            viewing_revision: None,
            viewing_revision_hash: None,
            file_at_revision: None,
            diff_rows: Vec::new(),
            content_title: None,
            focus: Focus::Tree,
            search: None,
            last_search_query: String::new(),
            in_file_search: None,
            in_file_search_open: false,
            json_query: None,
            json_query_original: Vec::new(),
            tree_filter: None,
            goto_line: None,
            command_usage: crate::command_usage::UsageStats::load(),
            command_palette: None,
            history: None,
            repo_log: None,
            theme_picker: None,
            recent_ring: Vec::new(),
            recent_files: None,
            bookmark_paths: Vec::new(),
            bookmarks: None,
            geo: Default::default(),
            context_menu: None,
            menu_bar_state: None,
            show_hidden: cfg.tree.show_hidden,
            ignore_gitignore: cfg.git.ignore_gitignore,
            tree_revision: 0,
            tree_width: cfg.tree.width,
            show_help: false,
            help_scroll: crate::scroll::ScrollState::new(),
            help_tab: 0,
            should_quit: false,
            tab_action_request: None,
            theme,
            git_status_enabled,
            git_show_deleted,
            git_show_untracked,
            git_show_ignored,
            git_info,
            worktree_count,
            git_status_map,
            git_staged_files,
            git_unstaged_files,
            git_untracked_files,
            git_mode: false,
            git_mode_flat: false,
            compare_base: None,
            commit_base: None,
            revision_picker: None,
            worktree_picker: None,
            bug_report: None,
            show_scrollbar: cfg.content.scrollbar,
            show_scroll_percentage: cfg.content.scroll_percentage,
            show_line_numbers: cfg.content.line_numbers,
            show_blame: false,
            blame_before_commit: false,
            show_about: false,
            show_welcome: false,
            walk_errors,
            config_error,
            auto_watch: cfg.content.watch,
            show_file_info: cfg.content.show_file_info,
            indent_guides: cfg.tree.indent_guides,
            icons_enabled: cfg.tree.icons,
            keys: cfg.keys,
            config_baseline: saved_config.clone(),
            config: saved_config,
            config_path,
            tree_visible_indices: None,
            tree_guide_cache: None,
            tree_affordance_cache: None,
            content_image: None,
            statusbar_segments: Vec::new(),
            last_click: None,
            last_breadcrumb_click: None,
            content_scrolled_at: Instant::now() - std::time::Duration::from_secs(10),
            highlighter,
            last_refresh: Instant::now(),
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
            symbols: HashMap::new(),
            file_diagnostics: HashMap::new(),
            file_diagnostic_lines: HashMap::new(),
            diagnostics_tx,
            diagnostics_rx,
            diagnostic_seq: 0,
            latest_diagnostic_requests: HashMap::new(),
            fold_display_map: Vec::new(),
            fold_gutter_rows: Vec::new(),
            yaml_error: None,
            yaml_anchor_count: 0,
            k8s_summary: None,
            yaml_alias_count: 0,
            loader,
            load_seq: 0,
            loading: false,
            git_seq: 0,
            rendered_content: HashMap::new(),
            rendered_text: HashMap::new(),
            cursor_positions: HashMap::new(),
            rendered_active: false,
            show_raw_markdown: false,
            rendered_active_path: None,
            status_message: None,
            content_highlight_cache: RefCell::new(None),
            is_log: false,
            is_jsonl: false,
            jsonl_source: Vec::new(),
            jsonl_expanded: HashSet::new(),
            jsonl_display_map: Vec::new(),
            follow_mode: false,
            follow_pinned: false,
            follow_candidate: None,
            follow_paused_until: None,
            log_highlight_cache: RefCell::new(std::collections::HashMap::new()),
            content_revision: 0,
            filter_query: None,
            filter_display_map: Vec::new(),
            filter_cache: RefCell::new(std::collections::HashMap::new()),
            filter_bar: None,
            session_dirty: false,
            session_dirty_at: None,
            session_last_save: Instant::now(),
            #[cfg(test)]
            clipboard_capture: Vec::new(),
            new_version_available,
            update_rx,
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
            if let Some(ref ir) = s.initial_root {
                if ir.exists() && *ir == app.root {
                    app.initial_root = ir.clone();
                }
            }
        }

        if has_session_override {
            app.rebuild(true);
        }

        // If the session specifies a file, select it in the tree.
        if let Some(ref s) = session_state {
            app.bookmark_paths = s
                .bookmarks
                .iter()
                .filter(|p| p.starts_with(&app.root) && p.is_file())
                .cloned()
                .collect();
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
