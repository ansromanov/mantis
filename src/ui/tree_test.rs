use super::*;
use crate::app::App;
use crate::config::Config;
use crate::git::GitStatus;
use crate::theme::Theme;
use crate::tree::TreeNode;
use ratatui::style::Color;
use std::collections::HashMap;
use std::path::PathBuf;

fn make_node(name: &str, is_dir: bool, deleted: bool) -> TreeNode {
    TreeNode {
        path: PathBuf::from(name),
        name: name.to_string(),
        depth: 0,
        is_dir,
        deleted,
    }
}

fn make_app(git_status_enabled: bool, status_map: HashMap<PathBuf, GitStatus>) -> App {
    let cfg = Config {
        git_status: false,
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    app.git_status_enabled = git_status_enabled;
    app.git_status_map = status_map;
    app
}

fn default_theme() -> Theme {
    Theme::default()
}

#[test]
fn git_status_deleted_file_uses_diff_del() {
    let node = make_node("gone.rs", false, true);
    let app = make_app(false, HashMap::new());
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().diff_del);
}

#[test]
fn git_status_new_file_uses_diff_add() {
    let node = make_node("new.rs", false, false);
    let mut map = HashMap::new();
    map.insert(PathBuf::from("new.rs"), GitStatus::New);
    let app = make_app(true, map);
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().diff_add);
}

#[test]
fn git_status_modified_file_uses_accent_alt() {
    let node = make_node("mod.rs", false, false);
    let mut map = HashMap::new();
    map.insert(PathBuf::from("mod.rs"), GitStatus::Modified);
    let app = make_app(true, map);
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().accent_alt);
}

#[test]
fn git_status_ignored_file_uses_dark_gray() {
    let node = make_node("ignored.log", false, false);
    let mut map = HashMap::new();
    map.insert(PathBuf::from("ignored.log"), GitStatus::Ignored);
    let app = make_app(true, map);
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, Color::DarkGray);
}

#[test]
fn git_status_regular_file_uses_file_color() {
    let node = make_node("plain.txt", false, false);
    let app = make_app(false, HashMap::new());
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().file);
}

#[test]
fn git_status_regular_dir_uses_dir_color_and_bold() {
    let node = make_node("mydir", true, false);
    let app = make_app(false, HashMap::new());
    let (color, bold) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().dir);
    assert_eq!(bold, Modifier::BOLD);
}

#[test]
fn git_status_deleted_takes_precedence_over_git_status() {
    let node = make_node("gone.rs", false, true);
    let mut map = HashMap::new();
    map.insert(PathBuf::from("gone.rs"), GitStatus::New);
    let app = make_app(true, map);
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().diff_del);
}

#[test]
fn git_status_enabled_but_path_not_in_map_uses_default() {
    let node = make_node("unknown.rs", false, false);
    let map = HashMap::new();
    let app = make_app(true, map);
    let (color, _) = git_status_style(&node, &app, &default_theme());
    assert_eq!(color, default_theme().file);
}
