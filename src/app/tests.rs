use std::fs;

use super::*;
use crate::config::Config;
use crate::search::SearchMode;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Creates a temp directory tree:
///   sub/ (with c.txt), a.txt, b.txt, long.txt (50 lines)
fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_app_test_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("b.txt"), "hello\n").unwrap();
    fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

/// A temp git repo with one committed file plus an uncommitted change.
fn temp_git_tree() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_app_git_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    fs::write(dir.join("tracked.txt"), "one\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "-q", "-m", "add tracked"]);
    fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn file_history_opens_picker_and_shows_diff() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));

    // H opens the history picker.
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    assert!(app.history.is_some());
    assert!(!app.history.as_ref().unwrap().commits.is_empty());

    // Enter loads the diff into the content panel.
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.history.is_none());
    assert!(app.is_diff);
    assert!(app.content_title.is_some());
    assert!(app.content.iter().any(|l| l.starts_with("+two")));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_and_reveal_selects_file_in_tree() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Reveal a file nested inside a collapsed subdirectory.
    let nested = root.join("sub").join("c.txt");
    app.open_and_reveal(&nested);

    assert_eq!(app.current_file.as_deref(), Some(nested.as_path()));
    assert!(matches!(app.focus, Focus::Content));
    // The parent dir is expanded and the file node is selected.
    assert!(app.expanded.contains(&root.join("sub")));
    assert_eq!(
        app.nodes.get(app.tree_selected).map(|n| n.path.clone()),
        Some(nested)
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn failed_open_clears_stale_current_file() {
    let root = temp_tree();
    let mut app = app_for(&root);

    // Open a real file so current_file and the watcher are populated.
    let good = root.join("a.txt");
    app.open_file(&good);
    assert_eq!(app.current_file.as_deref(), Some(good.as_path()));
    assert!(app.file_watch_path.is_some());

    // Opening a missing file fails the read: current_file and the watcher must
    // be cleared rather than left pointing at the previously opened file.
    let missing = root.join("does-not-exist.txt");
    app.open_file(&missing);
    assert_eq!(app.current_file, None);
    assert!(app.file_watch_path.is_none());
    assert!(app.content.iter().any(|l| l.starts_with("[error:")));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn file_history_noop_without_git_history() {
    let root = temp_tree(); // not a git repo
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    assert!(app.history.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_picker_applies_preset() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert_eq!(app.theme.accent, crate::theme::Theme::default().accent);

    // `t` opens the picker.
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    assert!(app.theme_picker.is_some());

    // Filter to "monokai" and apply it.
    for c in "monokai".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

    assert!(app.theme_picker.is_none());
    assert_eq!(
        app.theme.accent,
        crate::theme::Theme::preset("monokai").unwrap().accent
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_picker_esc_cancels() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let before = app.theme.accent;
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.theme_picker.is_none());
    assert_eq!(app.theme.accent, before); // unchanged
    fs::remove_dir_all(&root).ok();
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn click(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), col, row)
}

fn full_rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    }
}

#[test]
fn rect_contains_checks_bounds() {
    let r = Rect {
        x: 2,
        y: 3,
        width: 4,
        height: 5,
    };
    assert!(rect_contains(r, 2, 3)); // top-left corner
    assert!(rect_contains(r, 5, 7)); // inside, near far corner
    assert!(!rect_contains(r, 6, 3)); // column == x + width
    assert!(!rect_contains(r, 2, 8)); // row == y + height
    assert!(!rect_contains(r, 1, 3)); // left of area
    assert!(!rect_contains(r, 2, 2)); // above area
}

#[test]
fn left_click_in_tree_opens_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.tree_offset = 0;
    app.focus = Focus::Content;

    let idx = app.nodes.iter().position(|n| !n.is_dir).unwrap();
    let path = app.nodes[idx].path.clone();
    app.handle_mouse(click(1, idx as u16));

    assert_eq!(app.tree_selected, idx);
    assert_eq!(app.current_file.as_deref(), Some(path.as_path()));
    assert!(matches!(app.focus, Focus::Tree));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn left_click_on_dir_toggles_expand() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.tree_offset = 0;

    let dir_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
    let dir_path = app.nodes[dir_idx].path.clone();
    let before = app.nodes.len();

    app.handle_mouse(click(1, dir_idx as u16));
    assert!(app.expanded.contains(&dir_path));
    assert!(app.nodes.len() > before, "child should become visible");

    app.handle_mouse(click(1, dir_idx as u16));
    assert!(!app.expanded.contains(&dir_path));
    assert_eq!(app.nodes.len(), before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn left_click_respects_scroll_offset() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.tree_offset = 1; // first visible row is node index 1

    app.handle_mouse(click(1, 0));
    assert_eq!(app.tree_selected, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn click_below_last_node_is_ignored() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.tree_offset = 0;
    app.tree_selected = 0;

    // Row far past the last node.
    app.handle_mouse(click(1, 18));
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_wheel_moves_tree_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.content_area = Rect {
        x: 100,
        y: 0,
        width: 40,
        height: 20,
    };
    app.tree_selected = 0;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.tree_selected, 1);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_wheel_scrolls_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = full_rect();
    app.tree_area = Rect {
        x: 100,
        y: 0,
        width: 40,
        height: 20,
    };

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.content_scroll, 3);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

fn open_file_search(app: &mut App) {
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.search.is_some());
    app.search_area = full_rect();
    app.search_offset = 0;
}

#[test]
fn search_single_click_selects_without_opening() {
    let root = temp_tree();
    let mut app = app_for(&root);
    open_file_search(&mut app);

    app.handle_mouse(click(1, 1));
    assert_eq!(app.search.as_ref().unwrap().selected, 1);
    assert!(app.search.is_some(), "single click should not open");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_double_click_opens_result() {
    let root = temp_tree();
    let mut app = app_for(&root);
    open_file_search(&mut app);

    let target = app.search.as_ref().unwrap().file_results[0].clone();
    app.handle_mouse(click(1, 0));
    app.handle_mouse(click(1, 0)); // second click, same row, within window

    assert!(
        app.search.is_none(),
        "double click should open and close search"
    );
    assert_eq!(app.current_file.as_deref(), Some(target.as_path()));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_click_on_different_row_does_not_open() {
    let root = temp_tree();
    let mut app = app_for(&root);
    open_file_search(&mut app);
    // Need at least two results for this to be meaningful.
    if app.search.as_ref().unwrap().results_len() >= 2 {
        app.handle_mouse(click(1, 0));
        app.handle_mouse(click(1, 1));
        assert!(
            app.search.is_some(),
            "clicks on different rows must not open"
        );
        assert_eq!(app.search.as_ref().unwrap().selected, 1);
    }
    fs::remove_dir_all(&root).ok();
}

// -- content scroll capping ------------------------------------------------

fn viewport(height: u16) -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 80,
        height,
    }
}

#[test]
fn g_key_stops_at_scroll_max_not_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.focus = Focus::Content;
    app.content_area = viewport(10); // scroll_max = 50 - 10 = 40

    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::empty()));
    assert_eq!(
        app.content_scroll, 40,
        "G should land at scroll_max, not total-1"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn j_key_stops_at_scroll_max() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.focus = Focus::Content;
    app.content_area = viewport(10); // scroll_max = 40

    for _ in 0..60 {
        app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    }
    assert_eq!(app.content_scroll, 40, "j must not scroll past scroll_max");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn page_down_stops_at_scroll_max() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.focus = Focus::Content;
    app.content_area = viewport(10); // scroll_max = 40

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(
        app.content_scroll, 40,
        "PageDown must not scroll past scroll_max"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_scroll_down_stops_at_scroll_max() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = viewport(10); // scroll_max = 40
    app.tree_area = Rect {
        x: 100,
        y: 0,
        width: 10,
        height: 10,
    };

    for _ in 0..20 {
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    }
    assert_eq!(
        app.content_scroll, 40,
        "mouse scroll must not exceed scroll_max"
    );
    fs::remove_dir_all(&root).ok();
}

// -- git mode -------------------------------------------------------------

/// Repo with:
///   committed.txt  – committed "original", working-tree modified to "modified"
///   unchanged.txt  – committed "stable", untouched (must stay invisible in git mode)
///   new.txt        – untracked
///   sub/nested.txt – committed "nested", working-tree modified (gives sub/ a status)
fn temp_git_with_changes() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_git_mode_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    fs::write(dir.join("committed.txt"), "original\n").unwrap();
    fs::write(dir.join("unchanged.txt"), "stable\n").unwrap();
    fs::write(dir.join("sub").join("nested.txt"), "nested\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    fs::write(dir.join("committed.txt"), "modified\n").unwrap();
    fs::write(dir.join("sub").join("nested.txt"), "nested modified\n").unwrap();
    fs::write(dir.join("new.txt"), "brand new\n").unwrap();
    dir.canonicalize().unwrap()
}

fn ctrl_g() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)
}

fn alt_g() -> KeyEvent {
    KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT)
}

#[test]
fn git_mode_filters_tree_to_changed_files() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());

    assert!(app.git_mode);
    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    // Changed items must appear.
    assert!(names.contains(&"committed.txt"), "nodes: {names:?}");
    assert!(names.contains(&"new.txt"), "nodes: {names:?}");
    // Unchanged file must be absent.
    assert!(!names.contains(&"unchanged.txt"), "nodes: {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_toggle_off_restores_unchanged_files() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g()); // on
    app.handle_key(ctrl_g()); // off

    assert!(!app.git_mode);
    assert!(!app.is_diff, "should restore file content view");
    assert!(
        app.nodes.iter().any(|n| n.name == "unchanged.txt"),
        "unchanged file must reappear after exiting git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_auto_expands_dirs_with_changes() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);
    assert!(
        !app.expanded.contains(&root.join("sub")),
        "sub/ starts collapsed"
    );

    app.handle_key(ctrl_g());

    assert!(
        app.expanded.contains(&root.join("sub")),
        "git mode must auto-expand dirs containing changes"
    );
    assert!(
        app.nodes.iter().any(|n| n.path.ends_with("nested.txt")),
        "nested changed file must be visible in git mode tree"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_opens_working_tree_diff() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());

    // Navigate past any leading directory nodes to land on a file.
    // (tree.rs sorts dirs first, so sub/ may be at index 0.)
    let file_idx = app
        .nodes
        .iter()
        .position(|n| !n.is_dir)
        .expect("git mode must have at least one file node");
    for _ in 0..file_idx {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    }

    assert!(
        app.is_diff,
        "selecting a file in git mode must show working-tree diff"
    );
    assert!(
        app.content_title
            .as_deref()
            .unwrap_or("")
            .contains("working diff"),
        "title was {:?}",
        app.content_title
    );
    assert!(
        app.content.iter().any(|l| l.starts_with('+')),
        "diff must contain added lines"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_navigation_shows_diff_for_each_file() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());
    // Move to the next file node.
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));

    assert!(
        app.is_diff,
        "navigation in git mode must keep showing diffs"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_flat_shows_depth_zero_files() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());
    app.handle_key(alt_g());

    assert!(app.git_mode_flat);
    assert!(
        app.nodes.iter().all(|n| n.depth == 0 && !n.is_dir),
        "flat mode must only have depth-0 file nodes"
    );
    // Root-level file appears as bare name; nested file as relative path.
    assert!(app.nodes.iter().any(|n| n.name == "committed.txt"));
    assert!(app.nodes.iter().any(|n| n.name.contains("nested.txt")));
    // Unchanged file still absent.
    assert!(!app.nodes.iter().any(|n| n.name.contains("unchanged.txt")));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_flat_toggle_returns_to_tree_view() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());
    app.handle_key(alt_g()); // flat
    app.handle_key(alt_g()); // back to tree

    assert!(app.git_mode);
    assert!(!app.git_mode_flat);
    assert!(
        app.nodes.iter().any(|n| n.is_dir),
        "tree view should include directory nodes"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_flat_key_is_noop_outside_git_mode() {
    let root = temp_git_with_changes();
    let mut app = app_for(&root);
    let count = app.nodes.len();

    app.handle_key(alt_g());

    assert!(!app.git_mode_flat);
    assert!(!app.git_mode);
    assert_eq!(app.nodes.len(), count, "tree must be unchanged");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_outside_repo_gives_empty_tree() {
    let root = temp_tree(); // not a git repo
    let mut app = app_for(&root);

    app.handle_key(ctrl_g());

    assert!(app.git_mode);
    assert!(
        app.nodes.is_empty(),
        "no git changes → empty filtered tree; got {} nodes",
        app.nodes.len()
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_config_starts_enabled() {
    let root = temp_git_with_changes();
    let cfg = Config {
        git_mode: true,
        ..Config::default()
    };
    let app = App::new(root.to_path_buf(), cfg, None, None).unwrap();

    assert!(app.git_mode);
    assert!(
        !app.nodes.iter().any(|n| n.name == "unchanged.txt"),
        "unchanged file must be absent when starting in git mode"
    );
    assert!(
        app.nodes.iter().any(|n| n.name == "committed.txt"),
        "changed file must be visible when starting in git mode"
    );
    fs::remove_dir_all(&root).ok();
}

// -- diff_line_style ------------------------------------------------------

#[test]
fn diff_line_style_hunk_header_uses_accent() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("@@ -1,3 +1,4 @@", &app.theme);
    assert_eq!(style.fg, Some(app.theme.accent));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_file_header_uses_dim_bold() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("+++ b/file.rs", &app.theme);
    assert_eq!(style.fg, Some(app.theme.dim));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_addition_uses_diff_add() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("+new line", &app.theme);
    assert_eq!(style.fg, Some(app.theme.diff_add));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_removal_uses_diff_del() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("-old line", &app.theme);
    assert_eq!(style.fg, Some(app.theme.diff_del));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_diff_meta_uses_dim() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style("diff --git a/file b/file", &app.theme);
    assert_eq!(style.fg, Some(app.theme.dim));
    let style2 = diff_line_style("index abc..def", &app.theme);
    assert_eq!(style2.fg, Some(app.theme.dim));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_line_style_plain_text_uses_default() {
    let root = temp_tree();
    let app = app_for(&root);
    let style = diff_line_style(" context line", &app.theme);
    assert_eq!(style.fg, None);
    fs::remove_dir_all(&root).ok();
}

// -- deleted_set -----------------------------------------------------------

#[test]
fn deleted_set_returns_empty_when_disabled() {
    let map = std::collections::HashMap::new();
    let result = deleted_set(&map, false);
    assert!(result.is_empty());
}

#[test]
fn deleted_set_filters_existing_files() {
    let root = temp_tree();
    let mut map = std::collections::HashMap::new();
    map.insert(root.join("a.txt"), crate::git::GitStatus::Deleted);
    let result = deleted_set(&map, true);
    assert!(result.is_empty(), "existing files are not in deleted set");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn deleted_set_includes_absent_deleted_files() {
    let root = temp_tree();
    let gone = root.join("gone.txt");
    let mut map = std::collections::HashMap::new();
    map.insert(gone.clone(), crate::git::GitStatus::Deleted);
    let result = deleted_set(&map, true);
    assert!(result.contains(&gone));
    fs::remove_dir_all(&root).ok();
}

// -- open_file -------------------------------------------------------------

#[test]
fn open_file_nonexistent_shows_error() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("nonexistent.txt"));
    assert!(app.content[0].starts_with("[error:"));
    assert!(app.highlighted.is_empty());
    assert!(!app.is_diff);
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_binary_shows_binary_placeholder() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let bin_path = root.join("data.bin");
    fs::write(&bin_path, [0u8, 1, 2, 3]).unwrap();
    app.open_file(&bin_path);
    assert_eq!(app.content, vec!["[binary file]"]);
    assert!(app.highlighted.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_empty_shows_empty_placeholder() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let empty_path = root.join("empty.txt");
    fs::write(&empty_path, "").unwrap();
    app.open_file(&empty_path);
    assert_eq!(app.content, vec!["[empty file]"]);
    fs::remove_dir_all(&root).ok();
}

// -- show_deleted ----------------------------------------------------------

#[test]
fn show_deleted_sets_placeholder() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("gone.rs");
    app.show_deleted(&path);
    assert_eq!(app.current_file.as_deref(), Some(path.as_path()));
    assert_eq!(app.content, vec!["[deleted]"]);
    assert!(!app.is_diff);
    assert!(app.highlighted.is_empty());
    assert!(app.in_file_search.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- reopen_file -----------------------------------------------------------

#[test]
fn reopen_file_preserves_scroll_and_raw_markdown() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let md_path = root.join("doc.md");
    fs::write(&md_path, "# A\n\nline1\nline2\nline3\nline4\n").unwrap();
    app.open_file(&md_path);
    app.content_scroll = 3;
    app.content_hscroll = 5;
    app.show_raw_markdown = true;

    app.reopen_file(&md_path);
    assert!(app.show_raw_markdown);
    assert_eq!(
        app.content_scroll,
        3.min(app.line_count().saturating_sub(1))
    );
    assert_eq!(app.content_hscroll, 5);
    fs::remove_dir_all(&root).ok();
}

// -- content_line_count ----------------------------------------------------

#[test]
fn content_line_count_markdown_rendered() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let md_path = root.join("readme.md");
    fs::write(&md_path, "# H1\n\npara\n").unwrap();
    app.open_file(&md_path);
    assert!(app.is_markdown);
    assert!(!app.show_raw_markdown);
    assert_eq!(app.line_count(), app.markdown_lines.len());
    fs::remove_dir_all(&root).ok();
}

// -- content_scroll_max ----------------------------------------------------

#[test]
fn content_scroll_max_with_multiple_lines() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    assert_eq!(app.content_scroll_max(), 40);
    fs::remove_dir_all(&root).ok();
}

// -- line_prefix_width -----------------------------------------------------

#[test]
fn line_prefix_width_zero_for_diff_and_markdown() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    assert_eq!(app.line_prefix_width(), 0);
    app.is_diff = false;
    app.is_markdown = true;
    app.show_raw_markdown = false;
    assert_eq!(app.line_prefix_width(), 0);
    fs::remove_dir_all(&root).ok();
}

// -- selection_text --------------------------------------------------------

#[test]
fn selection_text_empty_when_no_selection() {
    let root = temp_tree();
    let app = app_for(&root);
    assert_eq!(app.selection_text(), "");
    fs::remove_dir_all(&root).ok();
}

// -- clear_selection -------------------------------------------------------

#[test]
fn clear_selection_resets_selection_and_drag() {
    use crate::selection::TextSelection;
    let root = temp_tree();
    let mut app = app_for(&root);
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 5),
    });
    app.drag_start = Some((0, 0));
    app.clear_selection();
    assert!(app.selection.is_none());
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- handle_normal_key -----------------------------------------------------

#[test]
fn normal_key_quit_sets_should_quit() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
    assert!(app.should_quit);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_help_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.show_help);
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(app.show_help);
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(!app.show_help);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_toggle_hidden_reloads() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let before = app.show_hidden;
    app.handle_key(KeyEvent::new(KeyCode::Char('.'), KeyModifiers::ALT));
    assert_ne!(app.show_hidden, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_switch_panel() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert_eq!(app.focus, Focus::Tree);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.focus, Focus::Content);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.focus, Focus::Tree);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_esc_clears_selection() {
    use crate::selection::TextSelection;
    let root = temp_tree();
    let mut app = app_for(&root);
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 1),
    });
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.selection.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- handle_search_key -----------------------------------------------------

#[test]
fn search_key_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.search.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.search.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_tab_toggles_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().mode, SearchMode::Content);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().mode, SearchMode::Files);
    fs::remove_dir_all(&root).ok();
}

// -- handle_in_file_search_key ---------------------------------------------

#[test]
fn in_file_search_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.in_file_search.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.in_file_search.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_enter_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.in_file_search.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.in_file_search.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- handle_content_key ----------------------------------------------------

#[test]
fn content_key_toggle_wrap_resets_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_scroll = 10;
    app.content_hscroll = 5;
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
    assert!(app.word_wrap);
    assert_eq!(app.content_scroll, 0);
    assert_eq!(app.content_hscroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_g_top_and_g_bottom() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = viewport(10);
    app.content_scroll = 20;
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::empty()));
    assert_eq!(app.content_scroll, 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::empty()));
    assert_eq!(app.content_scroll, app.content_scroll_max());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_zero_resets_hscroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_hscroll = 20;
    app.handle_key(KeyEvent::new(KeyCode::Char('0'), KeyModifiers::empty()));
    assert_eq!(app.content_hscroll, 0);
    fs::remove_dir_all(&root).ok();
}

// -- mouse drag in content area --------------------------------------------

#[test]
fn mouse_drag_selects_text_in_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 20,
    };

    let down = mouse(MouseEventKind::Down(MouseButton::Left), 6, 6);
    app.handle_mouse(down);
    assert_eq!(app.focus, Focus::Content);
    assert!(app.drag_start.is_some());

    let drag = mouse(MouseEventKind::Drag(MouseButton::Left), 10, 6);
    app.handle_mouse(drag);
    assert!(app.selection.is_some());

    let up = mouse(MouseEventKind::Up(MouseButton::Left), 10, 6);
    app.handle_mouse(up);
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_click_in_diff_does_not_start_drag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.is_diff = true;
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 20,
    };

    let down = mouse(MouseEventKind::Down(MouseButton::Left), 6, 6);
    app.handle_mouse(down);
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- search mouse ----------------------------------------------------------

#[test]
fn search_mouse_scroll_down_up() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.search_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    app.search_offset = 0;

    if app.search.as_ref().unwrap().results_len() >= 2 {
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.search.as_ref().unwrap().selected, 1);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.search.as_ref().unwrap().selected, 0);
    }
    fs::remove_dir_all(&root).ok();
}

// -- history mouse ---------------------------------------------------------

#[test]
fn history_mouse_scroll_down_up() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.history_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    app.history_offset = 0;

    if app.history.as_ref().unwrap().results_len() >= 2 {
        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.history.as_ref().unwrap().selected, 1);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.history.as_ref().unwrap().selected, 0);
    }
    fs::remove_dir_all(&root).ok();
}

// -- history mouse ---------------------------------------------------------

#[test]
fn theme_mouse_scroll_down_up() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.theme_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    app.theme_offset = 0;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 1);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

// -- command_palette -------------------------------------------------------

#[test]
fn command_palette_ctrl_p_opens() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.command_palette.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.command_palette.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_enter_executes_and_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let help_before = app.show_help;
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    // Filter for "help" so "Toggle help" is selected
    for c in "help".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.command_palette.is_none());
    assert_ne!(app.show_help, help_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_navigation() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 1);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_type_filters() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    let total = app.command_palette.as_ref().unwrap().results_len();
    app.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::empty()));
    assert!(app.command_palette.as_ref().unwrap().results_len() < total);
    fs::remove_dir_all(&root).ok();
}

// -- command_palette mouse -------------------------------------------------

#[test]
fn command_palette_mouse_scroll_down_up() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    app.command_palette_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    app.command_palette_offset = 0;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 1);
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

// --- content_pos ----------------------------------------------------------

#[test]
fn content_pos_no_wrap_plain_text() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt")); // "line1\nline2\n"
    app.word_wrap = false;
    app.content_scroll = 0;
    app.content_hscroll = 0;
    app.content_area = Rect {
        x: 2,
        y: 3,
        width: 80,
        height: 20,
    };
    // Row 3 = content_area.y, so rel_row = 0 -> buf_line = 0.
    // Col 2 = content_area.x, rel_col = 0, prefix = 2 (1-digit + space), buf_col = 0.
    let (line, _col) = app.content_pos(2, 3);
    assert_eq!(line, 0);
    // Row 4 -> rel_row = 1 -> buf_line = 1.
    let (line, _col) = app.content_pos(2, 4);
    assert_eq!(line, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_first_visual_row_of_first_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Markdown mode: prefix = 0.
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        // Line 0: 5 chars -> ceil(5/10) = 1 visual row.
        vec![(ratatui::style::Style::default(), "hello".to_string())],
        // Line 1: 22 chars -> ceil(22/10) = 3 visual rows.
        vec![(
            ratatui::style::Style::default(),
            "a long line that wraps".to_string(),
        )],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 2,
        y: 2,
        width: 10,
        height: 20,
    };
    // Mouse at (col=5, row=2) -> rel_row=0, rel_col=3, text_col=3.
    // Line 0: visual_rows=1, visual_remaining=0 < 1 -> (0, 0*10+3) = (0, 3).
    let (line, col) = app.content_pos(5, 2);
    assert_eq!(line, 0);
    assert_eq!(col, 3);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_first_visual_row_of_second_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "hello".to_string())],
        vec![(
            ratatui::style::Style::default(),
            "a long line that wraps".to_string(),
        )],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // rel_row=1: Line 0 uses 1 visual row (visual_remaining->0); Line 1: 0 < 3 -> (1, 0*10+4) = (1, 4).
    let (line, col) = app.content_pos(4, 1);
    assert_eq!(line, 1);
    assert_eq!(col, 4);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_second_visual_row_of_second_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "hello".to_string())],
        vec![(
            ratatui::style::Style::default(),
            "a long line that wraps".to_string(),
        )],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // rel_row=2: Line 0 uses 1 row; Line 1: visual_remaining=1 < 3 -> (1, 1*10+4) = (1, 14).
    let (line, col) = app.content_pos(4, 2);
    assert_eq!(line, 1);
    assert_eq!(col, 14);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_third_visual_row_of_second_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "hello".to_string())],
        vec![(
            ratatui::style::Style::default(),
            "a long line that wraps".to_string(),
        )],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // rel_row=3: Line 0 uses 1 row; Line 1: visual_remaining=2 < 3 -> (1, 2*10+4) = (1, 24).
    let (line, col) = app.content_pos(4, 3);
    assert_eq!(line, 1);
    assert_eq!(col, 24);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_past_all_content_clamps_to_last_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "hello".to_string())],
        vec![(
            ratatui::style::Style::default(),
            "a long line that wraps".to_string(),
        )],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // rel_row=10: past all 4 visual rows -> clamped to last logical line (1).
    let (line, _col) = app.content_pos(0, 10);
    assert_eq!(line, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_empty_line_counts_as_one_visual_row() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "".to_string())],
        vec![(ratatui::style::Style::default(), "next".to_string())],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // Empty line occupies 1 visual row; rel_row=1 -> second logical line.
    let (line, _col) = app.content_pos(0, 1);
    assert_eq!(line, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_respects_content_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "line0".to_string())],
        vec![(ratatui::style::Style::default(), "line1".to_string())],
        vec![(ratatui::style::Style::default(), "line2".to_string())],
    ];
    app.word_wrap = true;
    app.content_scroll = 1; // first visible logical line is index 1
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // rel_row=0 -> first visible line = logical line 1.
    let (line, _col) = app.content_pos(0, 0);
    assert_eq!(line, 1);
    // rel_row=1 -> logical line 2.
    let (line, _col) = app.content_pos(0, 1);
    assert_eq!(line, 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_plain_text_multi_span_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Plain-text mode with word_wrap.
    app.is_markdown = false;
    app.is_diff = false;
    app.content = vec![
        "short".to_string(),        // 5 chars -> 1 visual row with width 10
        "hello world!".to_string(), // 12 chars -> 2 visual rows with width 10
    ];
    // Override highlighted so line_prefix_width returns 1+1=2 (1-digit gutter).
    app.highlighted = vec![];
    app.word_wrap = true;
    app.content_scroll = 0;
    // prefix = line_prefix_width() = len("2") + 1 = 2; wrap_width = 10 - 2 = 8.
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 10,
        height: 20,
    };
    // "short" (5 chars) -> ceil(5/8) = 1 visual row.
    // "hello world!" (12 chars) -> ceil(12/8) = 2 visual rows.
    // rel_row=0 -> line 0.
    let (line, _col) = app.content_pos(2, 0);
    assert_eq!(line, 0);
    // rel_row=1 -> line 1, visual_remaining=0.
    let (line, col) = app.content_pos(2, 1);
    assert_eq!(line, 1);
    assert_eq!(col, 0); // text_col = 2 - prefix(2) = 0; 0*8 + 0 = 0
                        // rel_row=2 -> line 1, visual_remaining=1 -> col_offset = 1*8 = 8.
    let (line, col) = app.content_pos(2, 2);
    assert_eq!(line, 1);
    assert_eq!(col, 8);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_counts_wide_chars_by_display_width() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Markdown mode: prefix = 0, wrap_width = content_area.width = 8.
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        // 5 wide CJK glyphs: 5 chars but display width 10 -> ceil(10/8) = 2 rows.
        // Counting by chars would give ceil(5/8) = 1 row and mis-map the row below.
        vec![(
            ratatui::style::Style::default(),
            "\u{4e16}\u{754c}\u{4f60}\u{597d}\u{554a}".to_string(),
        )],
        vec![(ratatui::style::Style::default(), "next".to_string())],
    ];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 8,
        height: 20,
    };
    // rel_row=0 and rel_row=1 both fall on the wrapped first line.
    assert_eq!(app.content_pos(0, 0).0, 0);
    assert_eq!(app.content_pos(0, 1).0, 0);
    // rel_row=2 lands on the second logical line.
    assert_eq!(app.content_pos(0, 2).0, 1);
    fs::remove_dir_all(&root).ok();
}

// -- mark_content_scrolled -------------------------------------------------

#[test]
fn mark_content_scrolled_sets_timestamp() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let before = app.content_scrolled_at;
    std::thread::sleep(std::time::Duration::from_millis(1));
    app.mark_content_scrolled();
    assert!(app.content_scrolled_at > before);
    fs::remove_dir_all(&root).ok();
}
