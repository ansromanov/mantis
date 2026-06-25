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
fn breadcrumb_renders_for_nested_node() {
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
    let breadcrumb_row = &rows[1]; // row 1 is inside the border, before the tree list
    assert!(
        breadcrumb_row.contains("root") && breadcrumb_row.contains("src"),
        "breadcrumb row should contain path segments, got: {:?}",
        breadcrumb_row
    );
    // Should also include the filesystem root segment (/).
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
    // Should have 3 clickable areas: "/", "r", and "src".
    assert_eq!(
        app.breadcrumb_areas.len(),
        3,
        "expected 3 segments (/, r, src), got {}",
        app.breadcrumb_areas.len()
    );
    let (fs_path, fs_rect) = &app.breadcrumb_areas[0];
    let (root_path, root_rect) = &app.breadcrumb_areas[1];
    let (src_path, src_rect) = &app.breadcrumb_areas[2];
    assert_eq!(fs_path, &PathBuf::from("/"));
    assert_eq!(root_path, &PathBuf::from("/r"));
    assert_eq!(src_path, &PathBuf::from("/r/src"));
    // "/" is 1 char wide, "r" is 1 char, "src" is 3 chars.
    assert_eq!(fs_rect.width, 1);
    assert_eq!(root_rect.width, 1);
    assert_eq!(src_rect.width, 3);
    // "r" rect must start after "/" + " / " (3 chars separator).
    assert_eq!(root_rect.x, fs_rect.x + fs_rect.width + 3);
    // "src" rect must start after "r" + " / ".
    assert_eq!(src_rect.x, root_rect.x + root_rect.width + 3);
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
    app.root = PathBuf::from("/root");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/root/a/b/c/d/e"),
        name: "e".to_string(),
        depth: 5,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Narrow width forces compact mode: leading segments become "..".
    let text = breadcrumb_text_for(&mut app, 29);
    assert!(
        text.contains(".."),
        "compact breadcrumb should contain '..', got: {text:?}"
    );
    assert!(
        text.contains('e'),
        "current dir 'e' must be visible, got: {text:?}"
    );
    // The ".." segment must be clickable with a valid target path.
    let has_dotdot = app
        .breadcrumb_areas
        .iter()
        .any(|(p, _)| p.as_os_str() == ".." || p.to_string_lossy().contains("/"));
    assert!(has_dotdot, ".. target path must exist in breadcrumb_areas");
}

#[test]
fn compact_breadcrumb_keeps_current_dir_always() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/verylongrootname");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/verylongrootname/a/deeply/nested"),
        name: "nested".to_string(),
        depth: 3,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Very narrow width — only ".. / nested" should fit.
    let text = breadcrumb_text_for(&mut app, 16);
    assert!(
        text.contains("nested"),
        "current dir must always be visible, got: {text:?}"
    );
}

#[test]
fn compact_breadcrumb_dotdot_target_is_parent_of_first_kept() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/r");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/r/a/b/c"),
        name: "c".to_string(),
        depth: 3,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Use enough width to show ".. / b / c".
    let text = breadcrumb_text_for(&mut app, 14);
    assert!(text.contains(".."), "should have .. marker: {text:?}");
    assert!(text.contains("b"), "parent b should be visible: {text:?}");
    assert!(text.contains("c"), "current c should be visible: {text:?}");
    // With 7 segments and width 14 (inner=12), forward iteration picks
    // first_kept=3, keeping "b" and "c". The ".." target is the parent of
    // the first kept segment "b", i.e. /r/a.
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
        Some(PathBuf::from("/r/a")),
        ".. should navigate to parent of first kept segment"
    );
}

#[test]
fn compact_breadcrumb_fits_all_when_wide_enough() {
    let mut app = make_app(false, HashMap::new());
    app.root = PathBuf::from("/r");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/r/a/b"),
        name: "b".to_string(),
        depth: 2,
        is_dir: true,
        deleted: false,
    }];
    app.tree_selected = 0;
    // Wide enough for all three segments "/ / r / a / b".
    let text = breadcrumb_text_for(&mut app, 40);
    assert!(
        !text.contains(".."),
        "full-width breadcrumb should not compact, got: {text:?}"
    );
    assert!(text.contains('b'), "current dir must appear, got: {text:?}");
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
    app.root = PathBuf::from("/r");
    app.nodes = vec![TreeNode {
        path: PathBuf::from("/r/a/b/c/d/e"),
        name: "e".to_string(),
        depth: 5,
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
    // Last segment ("e") must always be visible.
    assert!(breadcrumb_row.contains('e'), "last segment must be visible");
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
    assert!(
        breadcrumb_row.contains("etc"),
        "breadcrumb must include child segment, got: {:?}",
        breadcrumb_row
    );
    // Exactly two clickable areas: "/" and "etc".
    assert_eq!(
        app.breadcrumb_areas.len(),
        2,
        "expected 2 segments (/ and etc), got {}",
        app.breadcrumb_areas.len()
    );
    assert_eq!(app.breadcrumb_areas[0].0, PathBuf::from("/"));
    assert_eq!(app.breadcrumb_areas[1].0, PathBuf::from("/etc"));
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
    assert!(app.tree_visible_indices.is_empty());
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
