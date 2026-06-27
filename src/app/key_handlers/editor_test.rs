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
        app.git_show_untracked, cfg.git_show_untracked,
        "git_show_untracked must match Config::default()"
    );
    assert_eq!(
        app.git_show_ignored, cfg.git_show_ignored,
        "git_show_ignored must match Config::default()"
    );
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
