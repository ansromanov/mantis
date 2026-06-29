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
fn dispatch_command_records_action_in_usage() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Force a known prior state so we can assert the change regardless of on-disk data.
    app.command_usage.record("reload");
    app.command_palette = Some(palette_with_query("Toggle help"));
    app.dispatch_command();
    assert_eq!(
        app.command_usage.last_used(),
        Some("toggle_help"),
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
