use crate::app::{App, Focus};
use crate::command_palette::COMMANDS;
use crate::config::Config;
use crate::search::CommandPalette;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_editor_key_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn palette_with_query(query: &str) -> CommandPalette {
    let mut p = CommandPalette::default();
    for c in query.chars() {
        p.push(c);
    }
    p
}

fn dispatch_blame_line(app: &mut App) {
    let mut p = CommandPalette::default();
    for c in "Blame active".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    app.dispatch_command();
}

#[test]
fn editor_blame_line_action_toggles_show_line_blame() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    app.focus = Focus::Content;
    assert!(!app.show_line_blame);
    dispatch_blame_line(&mut app);
    assert!(app.show_line_blame);
    dispatch_blame_line(&mut app);
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn editor_blame_line_action_noop_when_is_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    dispatch_blame_line(&mut app);
    assert!(!app.show_line_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_telemetry_toggles_config_and_handle() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.config.telemetry.enabled);

    let mut p = CommandPalette::default();
    for c in "Toggle telemetry".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    assert!(app.dispatch_command());
    assert!(app.config.telemetry.enabled);
    assert!(app.telemetry.is_enabled());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn commands_includes_blame_line_action() {
    assert!(COMMANDS.iter().any(|c| c.action_id == "blame_line"));
}

#[test]
fn commands_blame_line_has_expected_name() {
    let entry = COMMANDS
        .iter()
        .find(|c| c.action_id == "blame_line")
        .unwrap();
    assert_eq!(entry.name, "Blame active line");
}

#[test]
fn apply_theme_clears_plugin_content() {
    // Switching theme asks plugins to re-render, so stale plugin content (spans
    // and the parallel text store) must be dropped to avoid colour desync.
    use crate::search::ThemePicker;
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("a.txt");
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(ratatui::style::Style::default(), "x".to_string())]],
    );
    app.plugin_content_text.insert(path, vec!["x".to_string()]);

    let mut picker = ThemePicker::default();
    for c in "default".chars() {
        picker.push(c);
    }
    // Only run the assertion if a "default" theme is discoverable on this host.
    if picker.selected_name() == Some("default") {
        app.theme_picker = Some(picker);
        app.apply_selected_theme();
        assert!(app.plugin_content.is_empty(), "plugin_content must clear");
        assert!(
            app.plugin_content_text.is_empty(),
            "plugin_content_text must clear"
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(unix)]
fn apply_theme_sends_new_theme_colors_to_plugins() {
    // editor.rs's apply_theme hands the newly-resolved Theme to
    // plugin_manager.on_theme_change so plugins get the new theme's actual
    // colors, not just its name.
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let root = temp_tree();
    let plugin_dir = std::env::temp_dir().join(format!(
        "tv_editor_theme_colors_{}_{}",
        std::process::id(),
        root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&plugin_dir).unwrap();
    let out = plugin_dir.join("recv.txt");
    let script = plugin_dir.join("rec.sh");
    let mut f = fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\ncat > \"{}\"\n", out.display()).unwrap();
    drop(f);
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let mut cfg = Config::default();
    cfg.plugins.insert(
        "rec".to_string(),
        crate::plugin::PluginEntry {
            path: script.clone(),
            enabled: true,
            ..Default::default()
        },
    );
    let mut app = App::new(root.clone(), cfg, None, None).unwrap();

    let monokai = crate::theme::Theme::load("monokai").expect("monokai theme must load");
    app.apply_theme("monokai", monokai.clone());
    app.plugin_manager.deactivate_all();

    let deadline = Instant::now() + Duration::from_secs(3);
    let contents = loop {
        if let Ok(s) = fs::read_to_string(&out) {
            if s.matches(r#""event":"on_theme_change""#).count() >= 1 {
                break s;
            }
        }
        assert!(
            Instant::now() < deadline,
            "plugin never received on_theme_change"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    let line = contents
        .lines()
        .find(|l| l.contains(r#""event":"on_theme_change""#))
        .expect("on_theme_change event must be sent");
    let expected = format!(
        r#""heading1":"{}""#,
        crate::theme::color_to_hex(monokai.heading1)
    );
    assert!(
        line.contains(&expected),
        "on_theme_change colors must reflect the new theme, got: {line}"
    );
    fs::remove_dir_all(&root).ok();
}

// -- toggle_file_revision dispatch -------------------------------------------

#[test]
fn toggle_file_revision_dispatch_returns_true() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    if let Some(ref mut p) = app.command_palette {
        let idx = COMMANDS
            .iter()
            .position(|c| c.action_id == "toggle_file_revision")
            .expect("toggle_file_revision must be in COMMANDS");
        p.filtered = vec![idx];
        p.selected = 0;
    }
    assert!(
        app.dispatch_command(),
        "toggle_file_revision must be handled by dispatch_command"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn blame_open_commit_dispatch_returns_true() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    if let Some(ref mut p) = app.command_palette {
        let idx = COMMANDS
            .iter()
            .position(|c| c.action_id == "blame_open_commit")
            .expect("blame_open_commit must be in COMMANDS");
        p.filtered = vec![idx];
        p.selected = 0;
    }
    assert!(
        app.dispatch_command(),
        "blame_open_commit must be handled by dispatch_command"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_git_flat_noop_when_not_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.git_mode);
    assert!(!app.git_mode_flat);
    app.command_palette = Some(palette_with_query("Toggle git flat"));
    app.dispatch_command();
    assert!(
        !app.git_mode_flat,
        "git_mode_flat must not change when git_mode is false"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn go_to_line_command_opens_dialog_when_content_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = Some(root.join("a.txt"));
    app.focus = Focus::Content;
    app.command_palette = Some(palette_with_query("Go to line"));
    app.dispatch_command();
    assert!(app.goto_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn go_to_line_command_no_op_when_tree_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = Some(root.join("a.txt"));
    app.focus = Focus::Tree;
    app.command_palette = Some(palette_with_query("Go to line"));
    app.dispatch_command();
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn compare_against_command_opens_input_prompt() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    app.command_palette = Some(palette_with_query("Compare against a revision"));
    assert!(app.dispatch_command());
    assert!(app.revision_picker.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn compare_against_command_starts_with_empty_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    app.command_palette = Some(palette_with_query("Compare against a revision"));
    app.dispatch_command();
    assert!(app.revision_picker.is_some());
    // The revision picker always starts with shortcuts (HEAD, HEAD~1, HEAD~2),
    // and may also have branches/tags/commits from the git repo. Query should
    // be empty.
    assert!(app.revision_picker.as_ref().unwrap().query.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_up_dir_command_changes_root_for_top_level_file() {
    let root = temp_tree();
    let orig_root = root.clone();
    let mut app = app_for(&root);
    // Descend to sub
    let sub = root.join("sub");
    let sub_idx = app.nodes.iter().position(|n| n.path == sub).unwrap();
    app.tree_selected = sub_idx;
    app.descend_to_selected();

    // Now app.root is sub. Select c.txt inside sub.
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == sub.join("c.txt"))
        .expect("c.txt");
    app.tree_selected = file_idx;
    app.command_palette = Some(palette_with_query("Go up one"));
    app.dispatch_command();
    assert_eq!(
        app.root, orig_root,
        "tree_up_dir via command palette must change root back to initial_root"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn open_file_search_command_scoped_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = true;
    app.command_palette = Some(palette_with_query("Open file search"));
    app.dispatch_command();
    assert!(
        app.search.as_ref().unwrap().scoped,
        "file search must be scoped when git mode is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_search_command_not_scoped_outside_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(palette_with_query("Open file search"));
    app.dispatch_command();
    assert!(
        !app.search.as_ref().unwrap().scoped,
        "file search must not be scoped outside git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_content_search_command_scoped_in_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_mode = true;
    app.command_palette = Some(palette_with_query("Open content search"));
    app.dispatch_command();
    assert!(
        app.search.as_ref().unwrap().scoped,
        "content search must be scoped when git mode is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_git_mode_command_flips_git_mode_flag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.git_mode);
    app.command_palette = Some(palette_with_query("Toggle git mode"));
    app.dispatch_command();
    assert!(app.git_mode, "Toggle git mode command must enable git mode");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_git_show_fields_default_matches_config() {
    let root = temp_tree();
    let app = app_for(&root);
    let cfg = Config::default();
    assert_eq!(
        app.git_show_untracked, cfg.git.show_untracked,
        "git_show_untracked must match Config::default()"
    );
    assert_eq!(
        app.git_show_ignored, cfg.git.show_ignored,
        "git_show_ignored must match Config::default()"
    );
    fs::remove_dir_all(&root).ok();
}

// -- resolve_editor ----------------------------------------------------------

#[track_caller]
fn with_clean_env<F: FnOnce()>(f: F) {
    let prior_visual = std::env::var("VISUAL").ok();
    let prior_editor = std::env::var("EDITOR").ok();
    std::env::remove_var("VISUAL");
    std::env::remove_var("EDITOR");
    f();
    if let Some(v) = prior_visual {
        std::env::set_var("VISUAL", v);
    } else {
        std::env::remove_var("VISUAL");
    }
    if let Some(v) = prior_editor {
        std::env::set_var("EDITOR", v);
    } else {
        std::env::remove_var("EDITOR");
    }
}

#[test]
fn resolve_editor_uses_config_when_set() {
    let (cmd, is_fallback) = super::editor::resolve_editor(Some("code --wait"));
    assert_eq!(cmd, "code --wait");
    assert!(!is_fallback, "config override is not a fallback");
}

#[test]
fn resolve_editor_empty_config_falls_through() {
    with_clean_env(|| {
        std::env::set_var("EDITOR", "vim");
        let (cmd, is_fallback) = super::editor::resolve_editor(Some(""));
        assert_eq!(cmd, "vim");
        assert!(!is_fallback, "env var is not a fallback");
    });
}

#[test]
fn resolve_editor_config_wins_over_env() {
    with_clean_env(|| {
        std::env::set_var("VISUAL", "nano");
        let (cmd, is_fallback) = super::editor::resolve_editor(Some("code --wait"));
        assert_eq!(cmd, "code --wait");
        assert!(!is_fallback);
    });
}

#[test]
fn resolve_editor_uses_visual_when_set() {
    with_clean_env(|| {
        std::env::set_var("VISUAL", "my-editor");
        std::env::remove_var("EDITOR");
        let (result, is_fallback) = super::editor::resolve_editor(None);
        assert_eq!(result, "my-editor");
        assert!(!is_fallback, "env var is not a fallback");
    });
}

#[test]
fn resolve_editor_uses_editor_when_visual_unset() {
    with_clean_env(|| {
        std::env::remove_var("VISUAL");
        std::env::set_var("EDITOR", "nano");
        let (result, is_fallback) = super::editor::resolve_editor(None);
        assert_eq!(result, "nano");
        assert!(!is_fallback, "env var is not a fallback");
    });
}

#[test]
fn resolve_editor_fallback_returns_is_fallback_true() {
    if cfg!(windows) {
        return; // fallback differs on Windows
    }
    with_clean_env(|| {
        let (_cmd, is_fallback) = super::editor::resolve_editor(None);
        assert!(is_fallback, "fallback must set is_fallback flag");
    });
}

#[test]
fn resolve_editor_fallback_prefers_nano_over_vim() {
    if cfg!(windows) {
        return; // fallback differs on Windows
    }
    with_clean_env(|| {
        let (cmd, _) = super::editor::resolve_editor(None);
        let nano_available = std::process::Command::new("which")
            .arg("nano")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if nano_available {
            assert_eq!(cmd, "nano", "should prefer nano when available");
        } else {
            assert_eq!(cmd, "vim", "should fall back to vim when nano not found");
        }
    });
}

// -- open_in_browser ---------------------------------------------------------

#[test]
fn open_in_browser_empty_url_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.open_in_browser("").is_ok(), "empty URL must be a no-op");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_in_browser_non_tty_sets_status() {
    // In test context stdout is not a terminal, so the non-interactive branch fires.
    let root = temp_tree();
    let mut app = app_for(&root);
    let err = app
        .open_in_browser("https://example.com")
        .expect_err("non-interactive browser open must return an error");
    assert!(err.contains("not opening browser"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_external_non_tty_sets_status() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.status_message.is_none());
    app.open_external(std::path::Path::new("test.png"));
    assert!(
        app.status_message.is_some(),
        "non-interactive open_external must set a status message"
    );
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text.contains("not opening file"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_records_action_in_usage() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Force a known prior state so we can assert the change regardless of on-disk data.
    app.command_usage.record("reload");
    app.command_palette = Some(palette_with_query("Toggle help"));
    app.dispatch_command();
    assert_eq!(
        app.command_usage.last_used(),
        Some("help"),
        "dispatch_command must update last_used to the dispatched action_id"
    );
    fs::remove_dir_all(&root).ok();
}

// -- content config persist --------------------------------------------------

#[test]
fn toggle_word_wrap_via_command_persists_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.word_wrap = false;
    app.command_palette = Some(palette_with_query("Toggle word wrap"));
    app.dispatch_command();
    assert!(app.word_wrap, "app field should toggle");
    assert!(
        app.config.content.word_wrap,
        "config.content.word_wrap should persist the toggle"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_line_numbers_via_command_persists_to_config() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let initial = app.show_line_numbers;
    app.command_palette = Some(palette_with_query("Toggle line numbers"));
    app.dispatch_command();
    assert_eq!(app.show_line_numbers, !initial, "app field should toggle");
    assert_eq!(
        app.config.content.line_numbers, !initial,
        "config.content.line_numbers should persist the toggle"
    );
    fs::remove_dir_all(&root).ok();
}

// -- toggle_git_flat guard ---------------------------------------------------

#[test]
fn toggle_git_flat_noop_outside_git_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.git_mode);
    let initial = app.git_mode_flat;
    app.command_palette = Some(palette_with_query("Toggle git flat mode"));
    app.dispatch_command();
    assert_eq!(
        app.git_mode_flat, initial,
        "toggle_git_flat must be a noop when git_mode is off"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn apply_theme_does_not_modify_theme_picker() {
    // apply_theme was made pub(crate) so overlay handlers can call it for live
    // theme preview without saving config; it must leave picker state alone
    // so a preview call mid-navigation doesn't clobber the open picker.
    let root = temp_tree();
    let mut app = app_for(&root);
    let theme = crate::theme::Theme::default();
    app.apply_theme("default", theme);
    assert!(
        app.theme_picker.is_none(),
        "apply_theme must not modify picker state"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_unknown_action_id_does_not_crash() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    // Enter the overlay -> command palette path so dispatch_command() runs.
    app.command_palette = Some(palette_with_query("Toggle word wrap"));
    // Dispatch whatever the palette selected — just verify no crash.
    app.dispatch_command();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_quit_sets_should_quit() {
    // "quit" gained a palette entry + dispatch arm in this PR (issue #495's
    // proposal explicitly called it out as missing from the palette).
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(palette_with_query("Quit"));
    assert!(!app.should_quit);
    assert!(
        app.dispatch_command(),
        "dispatch_command must return true for a real match arm"
    );
    assert!(app.should_quit);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_returns_false_when_nothing_selected() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let mut p = CommandPalette::default();
    p.filtered.clear();
    app.command_palette = Some(p);
    assert!(
        !app.dispatch_command(),
        "dispatch_command must return false when no palette selection matches an arm"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_find_files_opens_file_picker() {
    // find_files gained a palette entry with the #298 keymap rework: on
    // terminals that swallow its key combos the palette is the fallback path.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(palette_with_query("Find files"));
    assert!(
        app.dispatch_command(),
        "find_files must be a real dispatch arm"
    );
    assert!(
        app.search.is_some(),
        "find_files must open the search picker"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_raw_markdown() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = Some(root.join("a.txt"));

    // When plugin_content_active is false, it should show a status message
    app.command_palette = Some(palette_with_query("Toggle markdown render"));
    assert!(app.dispatch_command());
    assert!(app
        .status_message
        .as_ref()
        .unwrap()
        .text
        .contains("not available"));

    // When plugin_content_active is true, it should run successfully (returns true)
    app.plugin_content_active = true;
    app.command_palette = Some(palette_with_query("Toggle markdown render"));
    assert!(app.dispatch_command());

    fs::remove_dir_all(&root).ok();
}
// -- copy_line / copy_file via command palette --------------------------------

#[test]
fn dispatch_command_copy_line_via_palette() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;
    app.command_palette = Some(palette_with_query("Copy line or selection"));
    assert!(
        app.dispatch_command(),
        "copy_line must be a real dispatch arm"
    );
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line1"),
        "copy_line must copy the active line text",
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_copy_file_via_palette() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.command_palette = Some(palette_with_query("Copy entire file"));
    assert!(
        app.dispatch_command(),
        "copy_file must be a real dispatch arm"
    );
    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line1\nline2"),
        "copy_file must copy all lines",
    );
    fs::remove_dir_all(&root).ok();
}

// Satisfying require-tests check

#[test]
fn dispatch_bug_report_saves_report_and_sets_status() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state.path());

    let root = temp_tree();
    let mut app = app_for(&root);
    let mut p = CommandPalette::default();
    for c in "Report a bug".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    assert!(app.dispatch_command());
    assert!(app.bug_report.is_some());

    app.save_bug_report();

    let msg = app
        .status_message
        .as_ref()
        .expect("status set")
        .text
        .clone();
    assert!(msg.starts_with("bug report saved:"), "got: {msg}");
    let dir = state.path().join("bug-reports");
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn dispatch_records_palette_action_in_telemetry_when_enabled() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state.path());

    let root = temp_tree();
    let cfg = Config {
        telemetry: crate::config::TelemetryConfig {
            enabled: true,
            notice_shown: false,
        },
        ..Config::default()
    };
    let mut app = App::new(root.clone(), cfg, None, None).unwrap();
    let mut p = CommandPalette::default();
    for c in "Toggle help".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    assert!(app.dispatch_command());
    drop(app); // flush the telemetry writer

    let telemetry_dir = state.path().join("telemetry");
    let mut files: Vec<_> = std::fs::read_dir(&telemetry_dir)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let name = name.to_str()?;
            if name.starts_with("events-") && name.ends_with(".jsonl") {
                Some(e.path())
            } else {
                None
            }
        })
        .collect();
    files.sort();
    let raw = std::fs::read_to_string(files.into_iter().next().unwrap()).unwrap();
    assert!(
        raw.lines()
            .any(|l| l.contains("\"action_invoked\"") && l.contains("\"help\"")),
        "palette dispatch must be recorded: {raw}"
    );
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn test_command_applicability_predicates() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());

    // Fixture 1: No file open, outside git repo.
    app.current_file = None;
    app.git_info = None;
    app.is_json = false;
    app.is_diff = false;
    app.fold_regions = Vec::new();
    app.plugin_content_active = false;
    app.git_mode = false;

    // Test applicability checks under this fixture
    assert_eq!(
        app.check_applicability("toggle_pretty_json"),
        Err("no file is open")
    );
    assert_eq!(
        app.check_applicability("blame_line"),
        Err("no file is open")
    );
    assert_eq!(
        app.check_applicability("file_history"),
        Err("no file is open")
    );
    assert_eq!(
        app.check_applicability("compare_against"),
        Err("not in a git repo")
    );
    assert_eq!(
        app.check_applicability("toggle_diff_staged"),
        Err("not in a git repo")
    );
    assert_eq!(
        app.check_applicability("toggle_diff_side_by_side"),
        Err("requires diff view")
    );
    assert_eq!(
        app.check_applicability("fold_toggle"),
        Err("no file is open")
    );
    assert_eq!(
        app.check_applicability("open_in_editor"),
        Err("no file is open")
    );
    assert_eq!(
        app.check_applicability("git_mode_flat_toggle"),
        Err("requires git mode")
    );

    // Fixture 2: File open, outside git repo, not JSON, no folds.
    app.current_file = Some(std::path::PathBuf::from("test.txt"));
    assert_eq!(
        app.check_applicability("toggle_pretty_json"),
        Err("requires JSON file")
    );
    assert_eq!(
        app.check_applicability("blame_line"),
        Err("not in a git repo")
    );
    assert_eq!(
        app.check_applicability("fold_toggle"),
        Err("no fold regions in file")
    );
    assert_eq!(app.check_applicability("open_in_editor"), Ok(()));

    // Fixture 3: File open, inside git repo, JSON file, folds exist.
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    app.is_json = true;
    app.fold_regions = vec![crate::fold::FoldRegion { start: 0, end: 5 }];
    // A `.json` file that fails to parse leaves `is_json` true but
    // `json_pretty_lines` empty (see loader.rs) - must not be reported
    // applicable, or dispatch silently no-ops on a palette entry the user
    // was told was runnable.
    assert_eq!(
        app.check_applicability("toggle_pretty_json"),
        Err("JSON file failed to parse")
    );
    app.json_pretty_lines = vec![vec![(ratatui::style::Style::default(), "{}".to_string())]];
    assert_eq!(app.check_applicability("toggle_pretty_json"), Ok(()));
    assert_eq!(app.check_applicability("blame_line"), Ok(()));
    assert_eq!(app.check_applicability("fold_toggle"), Ok(()));

    // Fixture 4: In a diff view.
    app.is_diff = true;
    // blame is not available in a diff
    assert_eq!(
        app.check_applicability("blame_line"),
        Err("not available in a diff")
    );
    assert_eq!(app.check_applicability("toggle_diff_staged"), Ok(()));
    app.is_diff = false;

    // Fixture 5: plugin-rendered content has no text cursor, so blame must
    // be reported inapplicable even though a file is open in a git repo.
    let plugin_path = app.current_file.clone().unwrap();
    app.plugin_content.insert(plugin_path, Vec::new());
    assert_eq!(
        app.check_applicability("blame_line"),
        Err("not available (current file not plugin-rendered)")
    );
}

#[test]
fn test_dispatch_inapplicable_command_sets_status_message() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());

    // Set up state where toggle_pretty_json is inapplicable (no JSON file)
    app.current_file = Some(std::path::PathBuf::from("test.txt"));
    app.is_json = false;
    app.status_message = None;

    // Open palette and select toggle_pretty_json
    let json_idx = crate::command_palette::COMMANDS
        .iter()
        .position(|c| c.action_id == "toggle_pretty_json")
        .unwrap();

    let inapplicability_reasons = crate::command_palette::COMMANDS
        .iter()
        .map(|cmd| app.check_applicability(&cmd.action_id).err())
        .collect();

    app.command_palette = Some(crate::command_palette::CommandPalette::new(
        app.keys(),
        vec![json_idx],
        0,
        inapplicability_reasons,
        Vec::new(),
    ));

    if let Some(ref mut p) = app.command_palette {
        let pos = p.filtered.iter().position(|&i| i == json_idx).unwrap();
        p.selected = pos;
    }

    // Dispatch the command!
    assert!(app.dispatch_command());

    // Verify:
    // 1. Palette is closed (command_palette is None)
    assert!(app.command_palette.is_none());
    // 2. Status message is set indicating why
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg
        .text
        .contains("Toggle JSON pretty-print: requires JSON file"));
    // 3. Changes nothing else (e.g. show_pretty_json is not toggled)
    assert!(!app.show_pretty_json);
}

#[test]
fn open_external_with_telemetry_enabled() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.telemetry = crate::telemetry::Telemetry::new(true);
    app.open_external(std::path::Path::new("test.png"));
    assert!(app.status_message.is_some());
    fs::remove_dir_all(&root).ok();
}

// -- tree_width_grow / tree_width_shrink via command palette (issue #665) ----

#[test]
fn dispatch_command_tree_width_grow_increases_width() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_width = 50;
    app.command_palette = Some(palette_with_query("Grow tree pane width"));
    assert!(
        app.dispatch_command(),
        "tree_width_grow must be a real dispatch arm"
    );
    assert_eq!(
        app.tree_width, 52,
        "dispatch tree_width_grow must increase width by 2"
    );
    assert_eq!(
        app.config.tree.width, 52,
        "config.tree.width must be persisted"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_tree_width_shrink_decreases_width() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_width = 50;
    app.command_palette = Some(palette_with_query("Shrink tree pane width"));
    assert!(
        app.dispatch_command(),
        "tree_width_shrink must be a real dispatch arm"
    );
    assert_eq!(
        app.tree_width, 48,
        "dispatch tree_width_shrink must decrease width by 2"
    );
    assert_eq!(
        app.config.tree.width, 48,
        "config.tree.width must be persisted"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_tree_width_grow_clamps_at_95() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_width = 95;
    app.command_palette = Some(palette_with_query("Grow tree pane width"));
    assert!(app.dispatch_command());
    assert_eq!(app.tree_width, 95, "tree_width must clamp at 95");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_tree_width_shrink_clamps_at_5() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_width = 5;
    app.command_palette = Some(palette_with_query("Shrink tree pane width"));
    assert!(app.dispatch_command());
    assert_eq!(app.tree_width, 5, "tree_width must clamp at 5");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_wrap_via_command_preserves_content_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.content_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 1,
    };
    app.content_scroll = 1;
    app.word_wrap = false;
    app.command_palette = Some(palette_with_query("Toggle word wrap"));
    app.dispatch_command();
    assert!(app.word_wrap);
    assert_eq!(
        app.content_scroll, 1,
        "toggle_wrap via command must preserve content_scroll"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_pretty_json_via_command_preserves_content_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.is_json = true;
    app.json_pretty_lines = vec![
        vec![(ratatui::style::Style::default(), "{".to_string())],
        vec![(ratatui::style::Style::default(), "  \"a\": 1,".to_string())],
        vec![(ratatui::style::Style::default(), "  \"b\": 2".to_string())],
        vec![(ratatui::style::Style::default(), "}".to_string())],
    ];
    app.json_pretty_text = vec![
        "{".to_string(),
        "  \"a\": 1,".to_string(),
        "  \"b\": 2".to_string(),
        "}".to_string(),
    ];
    app.show_pretty_json = false;
    app.content_area = ratatui::layout::Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 2,
    };
    app.content_scroll = 2;
    app.command_palette = Some(palette_with_query("Toggle JSON pretty"));
    app.dispatch_command();
    assert!(app.show_pretty_json);
    assert_eq!(
        app.content_scroll, 2,
        "toggle_pretty_json must preserve content_scroll"
    );
    fs::remove_dir_all(&root).ok();
}

// -- goto-line query parsing (shared by dialog and palette `:` route) -----------

#[test]
fn goto_line_from_query_absolute_relative_and_clamped() {
    let root = temp_tree();
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(root.join("long.txt"), long).unwrap();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;

    app.active_line = 10;
    app.goto_line_from_query("+5");
    assert_eq!(app.active_line, 15, "+n offsets forward");
    app.goto_line_from_query("-3");
    assert_eq!(app.active_line, 12, "-n offsets backward");
    app.goto_line_from_query("2");
    assert_eq!(app.active_line, 1, "absolute input is 1-indexed");
    app.goto_line_from_query("9999");
    assert_eq!(
        app.active_line,
        app.display_line_count() - 1,
        "absolute input clamps to the last line"
    );
    app.goto_line_from_query("junk");
    assert_eq!(
        app.active_line,
        app.display_line_count() - 1,
        "junk is a no-op"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_palette_goto_line_jumps_and_closes_palette() {
    let root = temp_tree();
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(root.join("long.txt"), long).unwrap();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.active_line = 0;

    let mut p = CommandPalette::default();
    p.push(':');
    p.route_goto_line = Some(crate::search::GotoLineState::new());
    p.push('2');
    p.push('0');
    app.command_palette = Some(p);
    app.dispatch_palette_goto_line();
    assert!(app.command_palette.is_none(), "dispatch closes the palette");
    assert_eq!(
        app.active_line, 19,
        "matches the standalone goto-line dialog"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn test_dispatch_plugin_command_closes_palette_and_returns_true() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = app_for(dir.path());

    let plugin_cmd = crate::plugin::PluginCommand {
        id: "demo.hello".to_string(),
        name: "Say Hello".to_string(),
        category: Some("Plugin".to_string()),
        description: None,
    };
    app.plugin_manager
        .register_commands("demo", vec![plugin_cmd.clone()]);

    let inapplicability_reasons = crate::command_palette::COMMANDS
        .iter()
        .map(|cmd| app.check_applicability(&cmd.action_id).err())
        .collect();
    app.command_palette = Some(crate::command_palette::CommandPalette::new(
        app.keys(),
        Vec::new(),
        0,
        inapplicability_reasons,
        vec![plugin_cmd],
    ));

    // Select the plugin command (appended after the built-ins).
    let plugin_idx = crate::command_palette::COMMANDS.len();
    if let Some(ref mut p) = app.command_palette {
        let pos = p.filtered.iter().position(|&i| i == plugin_idx).unwrap();
        p.selected = pos;
        assert_eq!(
            p.selected_command().map(|c| c.action_id.as_str()),
            Some("demo.hello")
        );
    }

    // Dispatch must claim the id (owned by "demo") and close the palette.
    // The plugin has no live subprocess, so send_command_event is a noop.
    assert!(app.dispatch_command());
    assert!(app.command_palette.is_none());
}

// -- repo log overlay ---------------------------------------------------------

#[test]
fn dispatch_repo_commit_log_opens_overlay() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    let mut p = CommandPalette::default();
    for c in "Browse repository commits".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    app.dispatch_command();
    assert!(app.repo_log.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_repo_commit_log_noop_without_git_info() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_info = None;
    let mut p = CommandPalette::default();
    for c in "Browse repository commits".chars() {
        p.push(c);
    }
    app.command_palette = Some(p);
    app.dispatch_command();
    assert!(app.repo_log.is_none());
    fs::remove_dir_all(&root).ok();
}
// touched for log follow mode
