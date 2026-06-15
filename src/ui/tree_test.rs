use super::*;
use crate::app::App;
use crate::config::Config;
use crate::git::GitStatus;
use crate::theme::Theme;
use crate::tree::TreeNode;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;
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

// ---------------------------------------------------------------------------
// draw_tree rendering tests
// ---------------------------------------------------------------------------

fn render_tree(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let area = Rect::new(0, 0, width, height);
    terminal.draw(|f| draw_tree(f, app, area)).unwrap();
    let buf = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

fn all_text(rows: &[String]) -> String {
    rows.join("")
}

#[test]
fn draw_tree_title_shows_root_file_name() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("myroot");
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(all_text(&rows).contains("myroot"));
}

#[test]
fn draw_tree_git_mode_shows_git_suffix() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(all_text(&rows).contains("[git]"));
}

#[test]
fn draw_tree_git_mode_flat_shows_flat_suffix() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.git_mode_flat = true;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(all_text(&rows).contains("[git:flat]"));
}

#[test]
fn draw_tree_no_git_mode_omits_git_suffix() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = false;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(!all_text(&rows).contains("[git]"));
}

#[test]
fn draw_tree_collapsed_dir_shows_right_arrow() {
    let mut app = make_app(false, HashMap::new());
    let node = make_node("src", true, false);
    app.nodes = vec![node];
    // expanded is empty, so "src" is collapsed
    let rows = render_tree(&mut app, 40, 5);
    assert!(all_text(&rows).contains('▶'), "collapsed dir must show ▶");
}

#[test]
fn draw_tree_expanded_dir_shows_down_arrow() {
    let mut app = make_app(false, HashMap::new());
    let node = make_node("src", true, false);
    app.expanded.insert(node.path.clone());
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    assert!(all_text(&rows).contains('▼'), "expanded dir must show ▼");
}

#[test]
fn draw_tree_file_node_has_no_arrow() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![make_node("main.rs", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(!text.contains('▶'), "file nodes must not have ▶");
    assert!(!text.contains('▼'), "file nodes must not have ▼");
    assert!(text.contains("main.rs"));
}

#[test]
fn draw_tree_depth_one_node_is_indented() {
    let mut app = make_app(false, HashMap::new());
    let mut node = make_node("mod.rs", false, false);
    node.depth = 1;
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    // Row 1 is the first content row (inside top border).
    // After the left border '│': 2-space indent (depth=1) + 2-space non-arrow = 4 spaces.
    let content_row = &rows[1];
    let after_border: String = content_row.chars().skip(1).take(4).collect();
    assert_eq!(
        after_border, "    ",
        "depth=1 file must be preceded by 4 spaces"
    );
}

#[test]
fn draw_tree_records_tree_area_geometry() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![];
    render_tree(&mut app, 40, 10);
    assert_eq!(app.tree_area.x, 1);
    assert_eq!(app.tree_area.y, 1);
    assert_eq!(app.tree_area.width, 38);
    assert_eq!(app.tree_area.height, 8);
}

#[test]
fn draw_tree_records_tree_offset_zero_on_first_render() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![];
    render_tree(&mut app, 40, 10);
    assert_eq!(app.tree_offset, 0);
}

#[test]
fn draw_tree_independent_scroll_preserves_scroll_position() {
    let mut app = make_app(false, HashMap::new());
    app.tree_independent_scroll = true;
    app.nodes = (0..20)
        .map(|i| {
            let mut n = make_node(&format!("file{i}.rs"), false, false);
            n.depth = 0;
            n
        })
        .collect();
    app.tree_scroll = 3;
    // height=6 gives view_height=4; max_scroll=20-4=16, so scroll=3 is valid
    render_tree(&mut app, 40, 6);
    assert_eq!(app.tree_scroll, 3);
    assert_eq!(app.tree_offset, 3);
}

#[test]
fn draw_tree_auto_scroll_keeps_selected_in_view() {
    let mut app = make_app(false, HashMap::new());
    app.tree_independent_scroll = false;
    app.nodes = (0..20)
        .map(|i| {
            let mut n = make_node(&format!("file{i}.rs"), false, false);
            n.depth = 0;
            n
        })
        .collect();
    app.tree_selected = 15;
    app.tree_scroll = 0;
    // height=6 → view_height=4; selection=15 must scroll into view
    render_tree(&mut app, 40, 6);
    // After render, scroll must bring row 15 into the 4-row viewport
    assert!(app.tree_scroll <= 15);
    assert!(app.tree_scroll + 4 > 15);
}
