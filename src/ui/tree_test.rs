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
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
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
fn draw_tree_title_shows_files_label() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("myroot");
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains("Files"),
        "title should show 'Files' mode label"
    );
    assert!(
        !all_text(&rows).contains("myroot"),
        "title must not contain root dir name"
    );
}

#[test]
fn draw_tree_git_mode_shows_git_label() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains("Git"),
        "title should show 'Git' in git mode"
    );
    assert!(
        !all_text(&rows).contains("[git]"),
        "title must not use old [git] badge"
    );
}

#[test]
fn draw_tree_git_mode_flat_shows_git_flat_label() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.git_mode_flat = true;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains("Git · flat"),
        "title should show 'Git · flat' in flat mode"
    );
    assert!(
        !all_text(&rows).contains("[git:flat]"),
        "title must not use old [git:flat] badge"
    );
}

#[test]
fn draw_tree_git_mode_clean_shows_placeholder() {
    use crate::git::GitHead;
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![];
    app.git_info = Some(crate::git::GitRepoInfo {
        head: GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('+'),
        "clean placeholder should show checkmark"
    );
    assert!(
        text.contains("Working tree clean"),
        "clean placeholder should show 'Working tree clean'"
    );
    assert!(
        text.contains("No changes to show."),
        "clean placeholder should show 'No changes to show.'"
    );
    assert!(
        text.contains("Ctrl+D"),
        "clean placeholder should show the exit binding"
    );
}

#[test]
fn draw_tree_git_mode_not_a_repo_shows_different_message() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![];
    app.git_info = None;
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('!'),
        "non-repo placeholder should show an error indicator"
    );
    assert!(
        text.contains("Not a git repository"),
        "non-repo placeholder should say 'Not a git repository'"
    );
    assert!(
        text.contains("No git data available."),
        "non-repo placeholder should say 'No git data available.'"
    );
}

#[test]
fn draw_tree_git_mode_with_nodes_no_placeholder() {
    use crate::git::GitHead;
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![make_node("changed.rs", false, false)];
    app.git_info = Some(crate::git::GitRepoInfo {
        head: GitHead::Branch("main".to_string()),
        ahead: 1,
        behind: 0,
        total_changed: 1,
        staged: 1,
        untracked: 0,
    });
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        !text.contains("Working tree clean"),
        "placeholder must not show when nodes are present"
    );
    assert!(
        !text.contains("Not a git repository"),
        "repo message must not show when nodes are present"
    );
    assert!(
        text.contains("changed.rs"),
        "file node should render instead of placeholder"
    );
}

#[test]
fn draw_tree_no_git_mode_shows_files_label() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("repo");
    app.git_mode = false;
    app.nodes = vec![];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains("Files"),
        "title should show 'Files' in normal mode"
    );
    assert!(
        !all_text(&rows).contains("Git"),
        "title must not show 'Git' in normal mode"
    );
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

fn make_app_with_icons() -> App {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_dir_open = "\u{f07c}".to_string();
    app.icon_dir_closed = "\u{f07b}".to_string();
    app.icon_map
        .insert("rs".to_string(), "\u{e795}".to_string());
    app
}

#[test]
fn draw_tree_collapsed_dir_hides_arrow_when_icons_active() {
    let mut app = make_app_with_icons();
    let node = make_node("src", true, false);
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        !text.contains('▶'),
        "collapsed dir must not show ▶ when icons active"
    );
    assert!(
        !text.contains('▼'),
        "collapsed dir must not show ▼ when icons active"
    );
}

#[test]
fn draw_tree_expanded_dir_hides_arrow_when_icons_active() {
    let mut app = make_app_with_icons();
    let node = make_node("src", true, false);
    app.expanded.insert(node.path.clone());
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        !text.contains('▶'),
        "expanded dir must not show ▶ when icons active"
    );
    assert!(
        !text.contains('▼'),
        "expanded dir must not show ▼ when icons active"
    );
}

#[test]
fn draw_tree_dirs_still_show_arrows_when_icons_disabled() {
    let mut app = make_app_with_icons();
    app.icons_enabled = false;
    let node = make_node("src", true, false);
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains('▶'),
        "collapsed dir must show ▶ when icons disabled"
    );
}

#[test]
fn draw_tree_dirs_still_show_arrows_when_icon_fields_empty() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    // icon maps are all empty — condition is false, arrows must remain
    let node = make_node("src", true, false);
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    assert!(
        all_text(&rows).contains('▶'),
        "collapsed dir must show ▶ when icon fields empty"
    );
}

#[test]
fn draw_tree_depth_one_node_is_indented() {
    let mut app = make_app(false, HashMap::new());
    let mut node = make_node("mod.rs", false, false);
    node.depth = 1;
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    // Row 1 is the breadcrumb bar, row 2 is the first tree content row (inside
    // top border). After the left border '│': 3-space indent guide cell (depth=1,
    // no siblings so no │) + 2-space non-arrow = 5 chars.
    let content_row = &rows[2];
    let after_border: String = content_row.chars().skip(1).take(5).collect();
    assert_eq!(
        after_border, "     ",
        "depth=1 file must be preceded by 5 chars (3 guide + 2 arrow placeholder)"
    );
}

#[test]
fn draw_tree_guide_cache_populated_after_render() {
    let mut app = make_app(false, HashMap::new());
    let mut dir_node = make_node("src", true, false);
    dir_node.depth = 0;
    let mut file_node = make_node("main.rs", false, false);
    file_node.depth = 1;
    app.nodes = vec![dir_node, file_node];
    assert!(app.tree_guide_cache.is_none());
    render_tree(&mut app, 40, 5);
    assert!(
        app.tree_guide_cache.is_some(),
        "guide masks must be cached after a render with indent_guides enabled"
    );
    let (rev, masks) = app.tree_guide_cache.as_ref().unwrap();
    assert_eq!(*rev, app.tree_revision);
    assert_eq!(masks.len(), 2);
}

#[test]
fn draw_tree_guide_shows_bar_when_later_sibling_follows_at_shallower_depth() {
    // dirA/fileA1 has a later depth-0 sibling (dirB), so its depth-0 guide
    // column must show a vertical bar. dirB/fileB1 is last at depth 0, so its
    // depth-0 column must be blank.
    let mut app = make_app(false, HashMap::new());
    let mut dir_a = make_node("dirA", true, false);
    dir_a.depth = 0;
    let mut file_a1 = make_node("a1.rs", false, false);
    file_a1.depth = 1;
    let mut dir_b = make_node("dirB", true, false);
    dir_b.depth = 0;
    let mut file_b1 = make_node("b1.rs", false, false);
    file_b1.depth = 1;
    app.nodes = vec![dir_a, file_a1, dir_b, file_b1];

    let rows = render_tree(&mut app, 40, 8);
    // a1.rs's row must carry a guide bar in its depth-0 guide cell (the 3
    // chars right after the left border); b1.rs's must not.
    let a1_row = rows.iter().find(|r| r.contains("a1.rs")).unwrap();
    let b1_row = rows.iter().find(|r| r.contains("b1.rs")).unwrap();
    let guide_cell = |row: &str| -> String { row.chars().skip(1).take(3).collect() };
    assert_eq!(
        guide_cell(a1_row),
        "│  ",
        "expected guide bar before a1.rs: {a1_row:?}"
    );
    assert_eq!(
        guide_cell(b1_row),
        "   ",
        "expected no guide bar before b1.rs: {b1_row:?}"
    );
}

#[test]
fn draw_tree_guide_cache_invalidated_on_tree_revision_bump() {
    // A stale cache keyed to an old tree_revision must be recomputed, not
    // reused, once the node list changes shape and the revision is bumped.
    let mut app = make_app(false, HashMap::new());
    let mut file_node = make_node("main.rs", false, false);
    file_node.depth = 1;
    app.nodes = vec![file_node];
    render_tree(&mut app, 40, 5);
    let (_, masks) = app.tree_guide_cache.as_ref().unwrap();
    assert_eq!(masks.len(), 1);

    app.tree_revision += 1;
    let mut file_node_1 = make_node("a.rs", false, false);
    file_node_1.depth = 1;
    let mut file_node_2 = make_node("b.rs", false, false);
    file_node_2.depth = 1;
    app.nodes = vec![file_node_1, file_node_2];
    render_tree(&mut app, 40, 5);
    let (rev, masks) = app.tree_guide_cache.as_ref().unwrap();
    assert_eq!(*rev, app.tree_revision);
    assert_eq!(
        masks.len(),
        2,
        "cache must be rebuilt for the new node count"
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
fn tree_auto_scroll_brings_selection_into_view() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = (0..20)
        .map(|i| {
            let mut n = make_node(&format!("file{i}.rs"), false, false);
            n.depth = 0;
            n
        })
        .collect();
    app.tree_selected = 15;
    app.tree_scroll = 0;
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 4,
    };
    // scroll_tree_into_view is called by keyboard Up/Down handlers;
    // it must nudge tree_scroll so the selected row fits the viewport.
    app.scroll_tree_into_view();
    assert!(app.tree_scroll <= 15);
    assert!(app.tree_scroll + 4 > 15);
}

#[test]
fn breadcrumb_shows_root_not_cursor_subdir() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/root/src/main.rs"),
        name: "main.rs".to_string(),
        depth: 2,
        is_dir: false,
        deleted: false,
    }];
    app.tree_selected = 0;
    let rows = render_tree(&mut app, 40, 6);
    let breadcrumb_row = &rows[1];
    assert!(
        breadcrumb_row.contains("root"),
        "breadcrumb should contain root name, got: {:?}",
        breadcrumb_row
    );
    assert!(
        !breadcrumb_row.contains("src"),
        "breadcrumb must not contain cursor subdir, got: {:?}",
        breadcrumb_row
    );
    assert!(
        breadcrumb_row.contains('/'),
        "breadcrumb should include filesystem root, got: {:?}",
        breadcrumb_row
    );
}

#[test]
fn breadcrumb_areas_match_rendered_columns() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/r");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/r/src"),
        name: "src".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    render_tree(&mut app, 40, 6);
    // Breadcrumb shows root + ancestors only: "/" and "r".
    assert_eq!(
        app.breadcrumb_areas.len(),
        2,
        "expected 2 segments (/, r), got {}",
        app.breadcrumb_areas.len()
    );
    let (fs_path, fs_rect) = &app.breadcrumb_areas[0];
    let (root_path, root_rect) = &app.breadcrumb_areas[1];
    assert_eq!(fs_path, &PathBuf::from("/"));
    assert_eq!(root_path, &PathBuf::from("/r"));
    assert_eq!(fs_rect.width, 1);
    assert_eq!(root_rect.width, 1);
    assert_eq!(root_rect.x, fs_rect.x + fs_rect.width + 1);
}

fn breadcrumb_text_for(app: &mut App, width: u16) -> String {
    let rows = render_tree(app, width, 6);
    rows[1].trim().to_string()
}

#[test]
fn render_breadcrumb_truncation_does_not_panic() {
    // Regression: old truncate_segments used segments.len() as an out-of-bounds
    // sentinel; compact_segments must never produce that.
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/r");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/r/a/b/c/d/e"),
        name: "e".to_string(),
        depth: 5,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    render_tree(&mut app, 20, 6);
    // Breadcrumb areas must contain only real segments — never a sentinel.
    for (path, _rect) in &app.breadcrumb_areas {
        assert!(!path.as_os_str().is_empty());
    }
}

#[test]
fn compact_breadcrumb_collapses_ancestors_to_dotdot() {
    let mut app = make_app(false, HashMap::new());
    // Deep root so breadcrumb has many ancestor segments to compact.
    app.root = PathBuf::from("/a/b/c/d/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/b/c/d/root/e"),
        name: "e".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Narrow width forces compact mode: leading segments become "..".
    let text = breadcrumb_text_for(&mut app, 16);
    assert!(
        text.contains(".."),
        "compact breadcrumb should contain '..', got: {text:?}"
    );
    assert!(
        text.contains("root"),
        "root name must be visible, got: {text:?}"
    );
    // The ".." target must be a proper ancestor of the root path.
    let root_path = PathBuf::from("/a/b/c/d/root");
    let dotdot = app.breadcrumb_areas.first().map(|(p, _)| p.clone());
    assert!(
        dotdot
            .clone()
            .is_some_and(|p| root_path.starts_with(&p) && p != root_path),
        ".. target must be a proper ancestor of root, got: {:?}",
        dotdot
    );
}

#[test]
fn compact_breadcrumb_keeps_root_always() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/a/longroot");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/longroot/x"),
        name: "x".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Narrow width — compact shows ".. / longroot".
    let text = breadcrumb_text_for(&mut app, 16);
    assert!(
        text.contains("longroot"),
        "root must always be visible, got: {text:?}"
    );
}

#[test]
fn compact_breadcrumb_dotdot_target_is_parent_of_first_kept() {
    let mut app = make_app(false, HashMap::new());
    // Deep root so we have multiple ancestor segments.
    app.root = PathBuf::from("/a/b/c/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/b/c/root/x"),
        name: "x".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Enough width to show ".. / b / c / root" (compact the first ancestor).
    // Inner width = 19-2 = 17, so first_kept=2 fits: total = 5+6+6 = 17 ≤ 17.
    let text = breadcrumb_text_for(&mut app, 19);
    assert!(text.contains(".."), "should have .. marker: {text:?}");
    assert!(text.contains("root"), "root should be visible: {text:?}");
    // The ".." target is the parent of the first kept segment (which is "b"
    // when first_kept=2: keeping "b", "c", "root"). Parent of "b" is "/a".
    let dotdot_target =
        app.breadcrumb_areas.iter().find_map(
            |(p, r)| {
                if r.width == 2 {
                    Some(p.clone())
                } else {
                    None
                }
            },
        );
    assert_eq!(
        dotdot_target,
        Some(PathBuf::from("/a")),
        ".. should navigate to parent of first kept segment"
    );
}

#[test]
fn compact_breadcrumb_fits_all_when_wide_enough() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/a/b/c/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/b/c/root/x"),
        name: "x".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Wide enough for all segments " / a / b / c / root".
    let text = breadcrumb_text_for(&mut app, 40);
    assert!(
        !text.contains(".."),
        "full-width breadcrumb should not compact, got: {text:?}"
    );
    assert!(text.contains("root"), "root must appear, got: {text:?}");
}

// -- icon rendering -----------------------------------------------------------

#[test]
fn draw_tree_icon_enabled_shows_icon_glyph() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_map
        .insert("rs".to_string(), "\u{e7a8}".to_string());
    app.icon_fallback = "\u{f15b}".to_string();
    app.nodes = vec![make_node("main.rs", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('\u{e7a8}'),
        "icon glyph should appear in tree row when icons_enabled"
    );
    assert!(
        text.contains("main.rs"),
        "filename must still appear next to the icon"
    );
}

#[test]
fn draw_tree_icon_enabled_fallback_for_unknown_ext() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_fallback = "\u{f15b}".to_string();
    app.nodes = vec![make_node("readme.md", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('\u{f15b}'),
        "unknown extension should use fallback icon"
    );
}

#[test]
fn draw_tree_icon_enabled_dir_shows_dir_icon() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_dir_open = "\u{f07c}".to_string();
    app.icon_dir_closed = "\u{f07b}".to_string();
    let node = make_node("src", true, false);
    app.expanded.insert(node.path.clone());
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('\u{f07c}'),
        "expanded dir should show dir_open icon"
    );
}

#[test]
fn draw_tree_icon_enabled_collapsed_dir_shows_closed_icon() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_dir_open = "\u{f07c}".to_string();
    app.icon_dir_closed = "\u{f07b}".to_string();
    let node = make_node("src", true, false);
    app.nodes = vec![node];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('\u{f07b}'),
        "collapsed dir should show dir_closed icon"
    );
}

#[test]
fn draw_tree_icon_enabled_empty_map_omits_icons() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_map.clear();
    app.nodes = vec![make_node("main.rs", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    // Icons disabled because map is empty, so just the filename
    assert!(text.contains("main.rs"));
}

#[test]
fn draw_tree_icons_disabled_omits_icon_glyph() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = false;
    app.icon_map
        .insert("rs".to_string(), "\u{e7a8}".to_string());
    app.nodes = vec![make_node("main.rs", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        !text.contains('\u{e7a8}'),
        "icon must not appear when icons_enabled is false"
    );
}

#[test]
fn draw_tree_icon_looks_up_extensionless_file_by_name() {
    let mut app = make_app(false, HashMap::new());
    app.icons_enabled = true;
    app.icon_map
        .insert("dockerfile".to_string(), "\u{e7b0}".to_string());
    app.icon_fallback = "\u{f15b}".to_string();
    app.nodes = vec![make_node("Dockerfile", false, false)];
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains('\u{e7b0}'),
        "extensionless file should look up by its full filename"
    );
}

#[test]
fn render_breadcrumb_truncation_shows_compact_dotdot() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/a/b/c/d/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/b/c/d/root/e"),
        name: "e".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    let rows = render_tree(&mut app, 20, 6);
    let breadcrumb_row = &rows[1];
    assert!(
        breadcrumb_row.contains(".."),
        "truncated breadcrumb must show '..', got: {:?}",
        breadcrumb_row
    );
    // Last segment (root) must always be visible.
    assert!(breadcrumb_row.contains("root"), "root must be visible");
}

#[test]
fn breadcrumb_when_root_is_filesystem_root() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/etc"),
        name: "etc".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    let rows = render_tree(&mut app, 40, 6);
    let breadcrumb_row = &rows[1];
    // When root is /, the single root segment must render as "/" without panic.
    assert!(
        breadcrumb_row.contains('/'),
        "breadcrumb must render the / root segment, got: {:?}",
        breadcrumb_row
    );
    // Breadcrumb shows only root (and ancestors), so no "etc" segment.
    assert!(
        !breadcrumb_row.contains("etc"),
        "breadcrumb must not include cursor subdir, got: {:?}",
        breadcrumb_row
    );
    // Exactly one clickable area: "/".
    assert_eq!(
        app.breadcrumb_areas.len(),
        1,
        "expected 1 segment (/), got {}",
        app.breadcrumb_areas.len()
    );
    assert_eq!(app.breadcrumb_areas[0].0, PathBuf::from("/"));
}

#[test]
fn breadcrumb_is_independent_of_cursor_position() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/a/b/mantis");
    app.nodes = vec![
        TreeNode {
            path: PathBuf::from("/a/b/mantis/docs"),
            name: "docs".to_string(),
            depth: 1,
            is_dir: true,
            deleted: false,
        },
        TreeNode {
            path: PathBuf::from("/a/b/mantis/src"),
            name: "src".to_string(),
            depth: 1,
            is_dir: true,
            deleted: false,
        },
    ];
    // Select "docs" — breadcrumb must end at "mantis", not include "docs".
    app.tree_selected = 0;
    let rows = render_tree(&mut app, 40, 6);
    let breadcrumb = &rows[1];
    assert!(
        breadcrumb.contains("mantis"),
        "root name must be in breadcrumb"
    );
    assert!(
        !breadcrumb.contains("docs"),
        "cursor subdir must not be in breadcrumb"
    );
    // Switch to "src" — breadcrumb must remain the same.
    app.tree_selected = 1;
    let rows2 = render_tree(&mut app, 40, 6);
    assert_eq!(
        rows[1], rows2[1],
        "breadcrumb must not change when cursor moves to different subdir"
    );
}

#[test]
fn compact_breadcrumb_dotdot_target_is_root_parent() {
    let mut app = make_app(false, HashMap::new());
    // Deep root so narrow tree forces compaction.
    app.root = PathBuf::from("/a/b/c/d/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/a/b/c/d/root/file.rs"),
        name: "file.rs".to_string(),
        depth: 1,
        is_dir: false,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Very narrow — only ".. / root" fits, falling back to the last segment's
    // parent = root.parent() = /a/b/c/d.
    render_tree(&mut app, 10, 6);
    let dotdot = app.breadcrumb_areas.first().map(|(p, _)| p.clone());
    assert_eq!(
        dotdot,
        Some(PathBuf::from("/a/b/c/d")),
        ".. must target root.parent(), got: {:?}",
        dotdot
    );
}

#[test]
fn breadcrumb_root_is_filesystem_root_renders_single_segment() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/home"),
        name: "home".to_string(),
        depth: 1,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Must render without panic. Breadcrumb is just "/".
    let rows = render_tree(&mut app, 40, 6);
    let breadcrumb_row = &rows[1];
    assert!(
        breadcrumb_row.contains('/'),
        "root segment / must render, got: {:?}",
        breadcrumb_row
    );
    assert_eq!(app.breadcrumb_areas.len(), 1);
    assert_eq!(app.breadcrumb_areas[0].0, PathBuf::from("/"));
}

// ---------------------------------------------------------------------------
// highlight_matches
// ---------------------------------------------------------------------------

#[test]
fn highlight_matches_no_match_returns_single_span() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    let spans = highlight_matches("foobar", "xyz", style, &theme);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "foobar");
}

#[test]
fn highlight_matches_exact_match_highlights_all() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    let spans = highlight_matches("foobar", "foobar", style, &theme);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "foobar");
    assert!(
        spans[0]
            .style
            .add_modifier
            .contains(ratatui::style::Modifier::BOLD),
        "matched span should be bold"
    );
}

#[test]
fn highlight_matches_partial_match_three_spans() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    // "foo_bar_baz" matching "bar" → "foo_" + "bar" + "_baz"
    let spans = highlight_matches("foo_bar_baz", "bar", style, &theme);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content, "foo_");
    assert_eq!(spans[1].content, "bar");
    assert_eq!(spans[2].content, "_baz");
}

#[test]
fn highlight_matches_case_insensitive() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    let spans = highlight_matches("HelloWorld", "world", style, &theme);
    assert_eq!(spans.len(), 2);
    assert_eq!(spans[0].content, "Hello");
    assert_eq!(spans[1].content, "World");
}

#[test]
fn highlight_matches_non_ascii_slices_on_char_boundary() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    // Multibyte chars: char index != byte index. Must not panic and must
    // split into prefix + match + suffix on character boundaries.
    let spans = highlight_matches("café_main.rs", "main", style, &theme);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content, "café_");
    assert_eq!(spans[1].content, "main");
    assert_eq!(spans[2].content, ".rs");
}

#[test]
fn highlight_matches_non_ascii_query_matches() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    let spans = highlight_matches("日本語.txt", "本語", style, &theme);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content, "日");
    assert_eq!(spans[1].content, "本語");
    assert_eq!(spans[2].content, ".txt");
}

#[test]
fn highlight_matches_empty_query_returns_single_span() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    let spans = highlight_matches("anything", "", style, &theme);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].content, "anything");
}

#[test]
fn highlight_matches_multiple_occurrences() {
    let theme = default_theme();
    let style = Style::default().fg(theme.file);
    // "aba" has "a" at positions 0 and 2
    let spans = highlight_matches("aba", "a", style, &theme);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].content, "a");
    assert_eq!(spans[1].content, "b");
    assert_eq!(spans[2].content, "a");
}

// ---------------------------------------------------------------------------
// draw_tree with tree_filter
// ---------------------------------------------------------------------------

#[test]
fn draw_tree_filter_shows_only_matching_nodes() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![
        make_node("main.rs", false, false),
        make_node("README.md", false, false),
        make_node("src", true, false),
    ];
    app.tree_filter = Some(crate::search::TreeFilter::new());
    app.tree_filter.as_mut().unwrap().push('m');
    app.tree_selected = 0;

    let rows = render_tree(&mut app, 40, 10);
    let text = all_text(&rows);
    assert!(text.contains("main.rs"), "main.rs should match 'm'");
    assert!(!text.contains("src"), "src should not match 'm'");
}

#[test]
fn draw_tree_filter_records_visible_indices() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![
        make_node("main.rs", false, false),
        make_node("README.md", false, false),
        make_node("src", true, false),
    ];
    app.tree_filter = Some(crate::search::TreeFilter::new());
    app.tree_filter.as_mut().unwrap().push('z');
    app.tree_selected = 0;

    render_tree(&mut app, 40, 10);
    // 'z' matches nothing, so visible_indices should be empty
    assert!(app.tree_visible_indices.as_ref().unwrap().is_empty());
}

#[test]
fn draw_tree_filter_empty_shows_all_nodes() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![
        make_node("main.rs", false, false),
        make_node("README.md", false, false),
    ];
    app.tree_filter = Some(crate::search::TreeFilter::new());

    let rows = render_tree(&mut app, 40, 10);
    let text = all_text(&rows);
    assert!(text.contains("main.rs"));
    assert!(text.contains("README.md"));
}

#[test]
fn draw_tree_filter_with_ancestors_includes_parent_dirs() {
    let mut app = make_app(false, HashMap::new());
    app.nodes = vec![
        TreeNode {
            path: PathBuf::from("/r/src"),
            name: "src".to_string(),
            depth: 0,
            is_dir: true,
            deleted: false,
        },
        TreeNode {
            path: PathBuf::from("/r/src/main.rs"),
            name: "main.rs".to_string(),
            depth: 1,
            is_dir: false,
            deleted: false,
        },
        TreeNode {
            path: PathBuf::from("/r/tests"),
            name: "tests".to_string(),
            depth: 0,
            is_dir: true,
            deleted: false,
        },
    ];
    app.tree_filter = Some(crate::search::TreeFilter::new());
    for c in "main".chars() {
        app.tree_filter.as_mut().unwrap().push(c);
    }

    let rows = render_tree(&mut app, 40, 10);
    let text = all_text(&rows);
    assert!(text.contains("main.rs"), "matching node must be visible");
    assert!(text.contains("src"), "ancestor dir must be visible");
    assert!(!text.contains("tests"), "non-matching dir must be hidden");
}

/// The git-mode clean placeholder's exit hint looks up the canonical
/// `git_mode_toggle` action id (see `crate::actions::ACTIONS`), not the old
/// `toggle_git_mode` alias the command palette used to use. A remapped
/// binding must show up here, proving the lookup uses the live keymap under
/// the canonical id.
#[test]
fn draw_tree_git_mode_placeholder_hint_uses_canonical_action_id() {
    use crate::config::{bind, Config, Keymap};
    use crate::git::GitHead;

    let cfg = Config {
        keys: Keymap {
            git_mode_toggle: bind(&["ctrl+m"]),
            ..Keymap::default()
        },
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    app.root = PathBuf::from("repo");
    app.git_mode = true;
    app.nodes = vec![];
    app.git_info = Some(crate::git::GitRepoInfo {
        head: GitHead::Branch("main".to_string()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    let rows = render_tree(&mut app, 40, 5);
    let text = all_text(&rows);
    assert!(
        text.contains("Ctrl+M"),
        "remapped git_mode_toggle binding must appear in the placeholder hint, got:\n{text}"
    );
}

// Modified for test requirements
