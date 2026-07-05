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
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
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
    fs::remove_dir_all(&plugin_dir).ok();
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
    app.focus = Focus::Tree;
    app.command_palette = Some(palette_with_query("Go to line"));
    app.dispatch_command();
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_up_dir_command_changes_root_for_top_level_file() {
    let root = temp_tree();
    let orig_root = root.clone();
    let mut app = app_for(&root);
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .expect("a.txt");
    app.tree_selected = file_idx;
    app.command_palette = Some(palette_with_query("Go up one"));
    app.dispatch_command();
    let parent = root.parent().expect("root has parent").to_path_buf();
    assert_eq!(
        app.root, parent,
        "tree_up_dir via command palette must change root"
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

#[test]
fn resolve_editor_uses_visual_when_set() {
    let prior_visual = std::env::var("VISUAL").ok();
    let prior_editor = std::env::var("EDITOR").ok();
    std::env::set_var("VISUAL", "my-editor");
    std::env::remove_var("EDITOR");
    let result = super::editor::resolve_editor();
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
    assert_eq!(result, "my-editor");
}

#[test]
fn resolve_editor_uses_editor_when_visual_unset() {
    let prior_visual = std::env::var("VISUAL").ok();
    let prior_editor = std::env::var("EDITOR").ok();
    std::env::remove_var("VISUAL");
    std::env::set_var("EDITOR", "nano");
    let result = super::editor::resolve_editor();
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
    assert_eq!(result, "nano");
}

#[test]
fn resolve_editor_fallback_vim_on_unix() {
    if cfg!(windows) {
        return; // fallback differs on Windows; tested separately
    }
    let prior_visual = std::env::var("VISUAL").ok();
    let prior_editor = std::env::var("EDITOR").ok();
    std::env::remove_var("VISUAL");
    std::env::remove_var("EDITOR");
    let result = super::editor::resolve_editor();
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
    assert_eq!(result, "vim");
}

// -- open_in_browser ---------------------------------------------------------

#[test]
fn open_in_browser_empty_url_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.status_message.is_none());
    app.open_in_browser("");
    assert!(
        app.status_message.is_none(),
        "empty URL must not set status"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_in_browser_non_tty_sets_status() {
    // In test context stdout is not a terminal, so the non-interactive branch fires.
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.status_message.is_none());
    app.open_in_browser("https://example.com");
    assert!(
        app.status_message.is_some(),
        "non-interactive browser open must set a status message"
    );
    let msg = app.status_message.as_ref().unwrap();
    assert!(msg.text.contains("not opening browser"));
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
