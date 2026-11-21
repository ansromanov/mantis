use std::collections::HashSet;
use std::fs;
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

// -- apply_reloaded_config / icons interaction --------------------------------

#[test]
fn config_reload_applies_icons_default_when_no_plugin_owns_icon_map() {
    let mut app = App {
        icons_enabled: true,
        ..create_base_app()
    };
    // No plugin_contributions entry with has_icon_map: icons_enabled came
    // from somewhere else (or stale state), so the config default should win.
    let cfg = crate::config::Config::default();
    app.apply_reloaded_config(cfg);

    assert!(
        !app.icons_enabled,
        "reload must apply cfg.tree.icons default when no plugin owns the icon map"
    );
}

// -- helpers ------------------------------------------------------------------

fn create_base_app() -> App {
    use crate::config::Config;
    use crate::highlight::Highlighter;
    use crate::theme::Theme;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::PathBuf;

    let theme = Theme::default();
    let highlighter = Highlighter::new(&theme.syntax);

    App {
        root: PathBuf::from("/tmp"),
        initial_root: PathBuf::from("/tmp"),
        nodes: Vec::new(),
        expanded: HashSet::new(),
        tree_selected: 0,
        tree_scroll: 0,
        tree_independent_scroll: false,
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
        active_line: 0,
        show_line_blame: false,
        word_wrap: false,
        current_file: None,
        current_syntax: None,
        is_diff: false,
        diff_side_by_side: false,
        viewing_revision: None,
        viewing_revision_hash: None,
        file_at_revision: None,
        diff_rows: Vec::new(),
        content_title: None,
        focus: crate::app::Focus::Tree,
        search: None,
        last_search_query: String::new(),
        in_file_search: None,
        in_file_search_open: false,
        json_query: None,
        json_query_original: Vec::new(),
        command_palette: None,
        history: None,
        repo_log: None,
        theme_picker: None,
        recent_ring: Vec::new(),
        recent_files: None,
        bookmark_paths: Vec::new(),
        bookmarks: None,
        recent_area: ratatui::layout::Rect::default(),
        recent_offset: 0,
        context_menu: None,
        context_menu_area: ratatui::layout::Rect::default(),
        menu_bar_state: None,
        menu_bar_area: ratatui::layout::Rect::default(),
        menu_dropdown_area: ratatui::layout::Rect::default(),
        show_hidden: false,
        ignore_gitignore: false,
        tree_revision: 0,
        tree_width: 28,
        show_help: false,
        help_scroll: crate::scroll::ScrollState::new(),
        help_tab: 0,
        help_area: ratatui::layout::Rect::default(),
        bug_report_area: ratatui::layout::Rect::default(),
        bug_report_preview_area: ratatui::layout::Rect::default(),
        should_quit: false,
        tab_action_request: None,
        theme,
        git_status_enabled: false,
        git_show_deleted: false,
        git_show_untracked: true,
        git_show_ignored: false,
        git_info: None,
        worktree_count: 0,
        git_status_map: HashMap::new(),
        git_staged_files: std::collections::HashSet::new(),
        git_unstaged_files: std::collections::HashSet::new(),
        git_untracked_files: std::collections::HashSet::new(),
        git_mode: false,
        git_mode_flat: false,
        compare_base: None,
        commit_base: None,
        revision_picker: None,
        worktree_picker: None,
        worktree_picker_area: ratatui::layout::Rect::default(),
        worktree_picker_offset: 0,
        revision_picker_area: ratatui::layout::Rect::default(),
        revision_picker_offset: 0,
        bug_report: None,
        show_scrollbar: false,
        show_scroll_percentage: false,
        show_line_numbers: false,
        show_blame: false,
        blame_before_commit: false,
        blame_area: ratatui::layout::Rect::default(),
        diff_header_area: ratatui::layout::Rect::default(),
        show_about: false,
        show_welcome: false,
        welcome_area: ratatui::layout::Rect::default(),
        walk_errors: 0,
        config_error: None,
        auto_watch: false,
        show_file_info: false,
        indent_guides: false,
        icons_enabled: false,
        keys: crate::config::Keymap::default(),
        config: Config::default(),
        config_baseline: Config::default(),
        config_path: None,
        tree_area: ratatui::layout::Rect::default(),
        tree_offset: 0,
        tree_visible_indices: None,
        tree_guide_cache: None,
        tree_affordance_cache: None,
        content_area: ratatui::layout::Rect::default(),
        content_image: None,
        image_area: ratatui::layout::Rect::default(),
        search_area: ratatui::layout::Rect::default(),
        search_offset: 0,
        command_palette_area: ratatui::layout::Rect::default(),
        command_palette_offset: 0,
        history_area: ratatui::layout::Rect::default(),
        history_offset: 0,
        repo_log_area: ratatui::layout::Rect::default(),
        repo_log_offset: 0,
        theme_area: ratatui::layout::Rect::default(),
        theme_offset: 0,
        splitter_area: ratatui::layout::Rect::default(),
        statusbar_area: ratatui::layout::Rect::default(),
        statusbar_segments: Vec::new(),
        last_click: None,
        last_breadcrumb_click: None,
        content_scrolled_at: std::time::Instant::now() - std::time::Duration::from_secs(10),
        highlighter,
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
        symbols: HashMap::new(),
        file_diagnostics: HashMap::new(),
        file_diagnostic_lines: HashMap::new(),
        diagnostics_tx: std::sync::mpsc::channel().0,
        diagnostics_rx: std::sync::mpsc::channel().1,
        diagnostic_seq: 0,
        latest_diagnostic_requests: HashMap::new(),
        fold_display_map: Vec::new(),
        fold_gutter_rows: Vec::new(),
        yaml_error: None,
        yaml_anchor_count: 0,
        k8s_summary: None,
        yaml_alias_count: 0,
        loader: crate::app::loader::Loader::new(&Theme::default(), usize::MAX),
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
        is_log: false,
        is_jsonl: false,
        jsonl_source: Vec::new(),
        jsonl_expanded: HashSet::new(),
        jsonl_display_map: Vec::new(),
        follow_mode: false,
        follow_pinned: false,
        follow_candidate: None,
        follow_paused_until: None,
        log_highlight_cache: RefCell::new(HashMap::new()),
        content_revision: 0,
        filter_query: None,
        filter_display_map: Vec::new(),
        filter_cache: RefCell::new(HashMap::new()),
        filter_bar: None,
        new_version_available: None,
        update_rx: None,
    }
}

// -- status message TTL tests --------------------------------------------------

// -- git status refresh tests -------------------------------------------------

// -- Supersession (stale worker responses discarded) --------------------------

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

// -- palette sub-picker debounce ------------------------------------------------

#[test]
fn tick_flushes_palette_content_search_debounce() {
    let dir = std::env::temp_dir().join(format!("tv_refresh_palette_{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "needle in here\n").unwrap();
    let dir = dir.canonicalize().unwrap();

    let mut app = create_base_app();
    let mut s = crate::search::SearchState::new(&dir, false, false, 0, None);
    s.toggle_mode(); // content mode
    for c in "needle".chars() {
        s.push(c); // only schedules a debounced refresh
    }
    let mut p = crate::command_palette::CommandPalette::default();
    p.push('#');
    p.route_search = Some(s);
    app.command_palette = Some(p);

    let results = |app: &App| {
        app.command_palette
            .as_ref()
            .and_then(|p| p.route_search.as_ref())
            .map(|s| s.content_results.len())
            .unwrap_or(0)
    };
    assert_eq!(results(&app), 0, "push only schedules the refresh");
    std::thread::sleep(Duration::from_millis(120));
    app.tick();
    assert!(
        results(&app) > 0,
        "tick must flush the palette sub-picker's debounced content refresh"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn reload_uses_range_status_when_compare_base_is_set() {
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
    fs::write(root.join("a.txt"), "hello\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    git(&["tag", "v0.1"]);

    // Modify a tracked file after the tag.
    fs::write(root.join("a.txt"), "modified\n").unwrap();

    let mut app = create_base_app();
    app.root = root.canonicalize().unwrap();
    app.git_status_enabled = true;
    app.git_mode = true;
    app.compare_base = Some("v0.1".to_string());

    assert!(app.git_status_map.is_empty());

    // Directly read the response to verify range_status is used.
    app.request_range_status("v0.1".to_string());
    let resp = app
        .loader
        .rx
        .recv_timeout(Duration::from_secs(5))
        .expect("worker should respond");
    if let LoadResponse::RangeStatus { load, error, .. } = resp {
        assert!(error.is_none(), "range_status should succeed: {error:?}");
        assert!(
            !load.status_map.is_empty(),
            "range_status must return modified files"
        );
    } else {
        panic!("expected RangeStatus response");
    }

    // Now verify that reload() dispatches range_status when compare_base is set.
    app.reload();
    // The reload dispatches range_status, not git_status_refresh, because
    // compare_base is Some. Pump to apply the response.
    app.pump_loads();

    let a = app.root.join("a.txt");
    assert!(
        app.git_status_map.contains_key(&a),
        "reload with compare_base must use range_status so changed files appear"
    );
}

#[test]
fn reload_uses_git_status_refresh_when_compare_base_is_none() {
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
    fs::write(root.join("a.txt"), "hello\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);

    // Uncommitted modification.
    fs::write(root.join("a.txt"), "modified\n").unwrap();

    let mut app = create_base_app();
    app.root = root.canonicalize().unwrap();
    app.git_status_enabled = true;
    app.git_mode = true;
    // compare_base is None — plain git mode, not compare mode.
    assert!(app.compare_base.is_none());

    app.reload();
    app.pump_loads();

    // The regular git status should show the uncommitted modification.
    let a = app.root.join("a.txt");
    assert!(
        app.git_status_map.contains_key(&a),
        "reload without compare_base must use git_status_refresh"
    );
}

#[test]
fn log_auto_follow_scrolls_to_bottom_on_grow() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("test.log");
    fs::write(&file_path, "line 1\n").unwrap();

    let mut app = create_base_app();
    app.current_file = Some(file_path.clone());
    app.is_log = true;
    app.follow_mode = true;
    app.follow_pinned = true;

    // Load file
    app.open_file(&file_path);
    assert_eq!(app.line_count(), 1);

    // Setup channel for watch events
    let (tx, rx) = std::sync::mpsc::channel();
    app.file_watch_rx = Some(rx);
    app.file_watch_path = Some(file_path.clone());

    // Append to file
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(&file_path)
        .unwrap();
    use std::io::Write;
    writeln!(file, "line 2").unwrap();
    writeln!(file, "line 3").unwrap();
    drop(file);

    // Send notify event
    let mut event = notify::Event::new(notify::EventKind::Modify(notify::event::ModifyKind::Data(
        notify::event::DataChange::Any,
    )));
    event.paths = vec![file_path.clone()];
    tx.send(Ok(event)).unwrap();

    // Trigger tick/update
    app.tick();

    // The file should have grown, and follow pinning should scroll it to the bottom
    assert_eq!(app.line_count(), 3);
    assert_eq!(app.active_line, 2);
}
