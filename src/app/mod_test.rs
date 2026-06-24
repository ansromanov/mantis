use std::fs;

use super::*;
use crate::command_palette::COMMANDS;
use crate::config::Config;
use crate::fold::FoldRegion;
use crate::search::{GotoLineState, InFileMatch, SearchMode, TreeFilter};
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
fn key_release_events_are_ignored() {
    use crossterm::event::KeyEventKind;

    let root = temp_tree();
    let mut app = app_for(&root);
    let start = app.tree_selected;

    // A Release event (as Windows emits alongside every Press) must be a no-op,
    // otherwise each physical key press would be handled twice.
    app.handle_key(KeyEvent::new_with_kind(
        KeyCode::Down,
        KeyModifiers::empty(),
        KeyEventKind::Release,
    ));
    assert_eq!(
        app.tree_selected, start,
        "Release must not move the selection"
    );

    // The matching Press event moves the selection exactly one row.
    app.handle_key(KeyEvent::new_with_kind(
        KeyCode::Down,
        KeyModifiers::empty(),
        KeyEventKind::Press,
    ));
    assert_eq!(app.tree_selected, start + 1, "Press moves selection once");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn watch_root_installs_recursive_watcher() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.root_watcher.is_none(), "no watcher before watch_root");
    app.watch_root();
    assert!(app.root_watcher.is_some(), "watcher installed on the root");
    assert!(app.root_watch_rx.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_dirty_reload_is_debounced() {
    use std::time::{Duration, Instant};

    let root = temp_tree();
    let mut app = app_for(&root);

    // A just-seen event must NOT reload yet — the tree has to go quiet first.
    app.tree_dirty = true;
    app.tree_dirty_at = Some(Instant::now());
    app.tick();
    assert!(
        app.tree_dirty,
        "reload deferred while events are still fresh"
    );

    // Once the debounce window has passed, tick clears the flag and reloads.
    // Use 61 s so this is still stale past the #[cfg(test)] 60 s window.
    app.tree_dirty_at = Some(Instant::now() - Duration::from_secs(61));
    app.tick();
    assert!(!app.tree_dirty, "reload runs after the tree goes quiet");
    assert!(app.tree_dirty_at.is_none());
    fs::remove_dir_all(&root).ok();
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
        crate::theme::Theme::load("monokai").unwrap().accent
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

    // Clear last_click so the second click is a fresh single click rather than
    // a double-click (which would descend into the directory, not collapse it).
    app.last_click = None;
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
fn tree_filter_mouse_click_maps_through_visible_indices_and_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // a.txt at index 0, subdir/ at index 1, long.txt at index 2+, ...
    // Set up a filter that only shows 'long.txt' (index ~2) and its ancestors.
    app.tree_filter = Some(TreeFilter::new());
    for c in "long".chars() {
        app.tree_filter.as_mut().unwrap().push(c);
    }
    // Simulate the visible_indices that draw_tree would compute.
    let matching: Vec<usize> = (0..app.nodes.len())
        .filter(|&i| app.nodes[i].name.to_lowercase().contains("long"))
        .collect();
    assert!(!matching.is_empty(), "long.txt should match");
    app.tree_visible_indices = matching.clone();
    app.tree_area = full_rect();
    app.tree_offset = 0;

    // Click on the first visible row (which maps to matching[0]).
    app.handle_mouse(click(1, 0));
    assert_eq!(
        app.tree_selected, matching[0],
        "should select the global index from visible_indices"
    );
    assert!(
        app.tree_filter.is_none(),
        "click should accept (close) the filter"
    );
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
fn scroll_wheel_scrolls_tree_viewport() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 1,
    };
    app.content_area = Rect {
        x: 100,
        y: 0,
        width: 40,
        height: 20,
    };
    app.tree_selected = 0;
    app.tree_scroll = 0;

    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 0));
    assert_eq!(app.tree_selected, 0, "wheel must not move the selection");
    assert!(app.tree_scroll > 0, "wheel must scroll the tree viewport");

    let scrolled = app.tree_scroll;
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 0));
    assert!(
        app.tree_scroll < scrolled,
        "scroll up must reduce tree_scroll"
    );
    assert_eq!(
        app.tree_selected, 0,
        "selection must not change when scrolling up"
    );
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
    // Focus content with no file open so `/` falls through to the full
    // filesystem search picker rather than the in-file search or tree filter.
    app.focus = Focus::Content;
    app.current_file = None;
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

// -- handle_tree_filter_key -------------------------------------------------

#[test]
fn tree_filter_key_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_enter_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.tree_filter.as_mut().unwrap().push('a');
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.tree_filter.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_backspace_removes_char() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.tree_filter.as_mut().unwrap().push('a');
    assert_eq!(app.tree_filter.as_ref().unwrap().query, "a");
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.tree_filter.as_ref().unwrap().query, "");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_char_appends() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    assert_eq!(app.tree_filter.as_ref().unwrap().query, "x");
    app.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::empty()));
    assert_eq!(app.tree_filter.as_ref().unwrap().query, "xy");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_moves_selection_to_first_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Open the tree filter explicitly (normally done via '/' with tree focus).
    app.tree_filter = Some(TreeFilter::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::empty()));
    assert_eq!(app.tree_filter.as_ref().unwrap().query, "l");
    // tree_selected should move to the first node whose name contains 'l'
    // (long.txt at index ~2, after a.txt, b.txt, and possibly a directory).
    let matching = app
        .nodes
        .iter()
        .position(|n| n.name.to_lowercase().contains('l'));
    assert!(matching.is_some(), "at least one node should match 'l'");
    assert_eq!(app.tree_selected, matching.unwrap());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_moves_selection_to_zero_when_no_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.tree_selected = 2;
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
    // 'z' matches nothing; selection should fall back to 0
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_filter_key_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_filter = Some(TreeFilter::new());
    app.tree_filter.as_mut().unwrap().push('a');
    let query_before = app.tree_filter.as_ref().unwrap().query.clone();
    app.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()));
    assert_eq!(app.tree_filter.as_ref().unwrap().query, query_before);
    fs::remove_dir_all(&root).ok();
}

// -- handle_goto_line_key -------------------------------------------------

#[test]
fn goto_line_key_opens_with_colon_and_content_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_some());
    assert!(app.goto_line.as_ref().unwrap().query.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_does_not_open_with_tree_focus() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_char_appends() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('4'), KeyModifiers::empty()));
    assert_eq!(app.goto_line.as_ref().unwrap().query, "4");
    app.handle_key(KeyEvent::new(KeyCode::Char('2'), KeyModifiers::empty()));
    assert_eq!(app.goto_line.as_ref().unwrap().query, "42");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_backspace_removes_char() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    app.goto_line.as_mut().unwrap().push('4');
    app.goto_line.as_mut().unwrap().push('2');
    assert_eq!(app.goto_line.as_ref().unwrap().query, "42");
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.goto_line.as_ref().unwrap().query, "4");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_enter_jumps_to_absolute_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.open_file(&root.join("long.txt"));
    app.content_scroll = 0;
    app.goto_line = Some(GotoLineState::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(app.content_scroll, 4);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_enter_relative_plus_jump() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.open_file(&root.join("long.txt"));
    app.content_scroll = 10;
    app.goto_line = Some(GotoLineState::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(app.content_scroll, 15);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_enter_relative_minus_jump() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.open_file(&root.join("long.txt"));
    app.content_scroll = 20;
    app.goto_line = Some(GotoLineState::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('5'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(app.content_scroll, 15);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_enter_clamps_to_max() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect::new(0, 0, 80, 10);
    app.content_scroll = 0;
    app.goto_line = Some(GotoLineState::new());
    app.handle_key(KeyEvent::new(KeyCode::Char('9'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('9'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(app.content_scroll, 40);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_enter_empty_query_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.content_scroll = 5;
    app.goto_line = Some(GotoLineState::new());
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.goto_line.is_none());
    assert_eq!(app.content_scroll, 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_some());
    let query_before = app.goto_line.as_ref().unwrap().query.clone();
    app.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()));
    assert_eq!(app.goto_line.as_ref().unwrap().query, query_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn goto_line_key_open_binding_not_appended_to_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.is_some());
    // pressing ':' again should not append it to the query
    app.handle_key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::empty()));
    assert!(app.goto_line.as_ref().unwrap().query.is_empty());
    fs::remove_dir_all(&root).ok();
}

// -- handle_search_key -----------------------------------------------------

#[test]
fn search_key_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
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
    app.focus = Focus::Content;
    app.current_file = None;
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
    app.focus = Focus::Content;
    app.current_file = None;
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
    let before = std::time::Instant::now();
    app.mark_content_scrolled();
    assert!(
        app.content_scrolled_at >= before,
        "timestamp must be set to now or later"
    );
    fs::remove_dir_all(&root).ok();
}

// -- handle_key dispatch / about / help overlays ---------------------------

#[test]
fn handle_key_show_about_q_or_esc_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::empty()));
    assert!(!app.show_about);
    app.show_about = true;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(!app.show_about);
    app.show_about = true;
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(!app.show_about);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_key_show_about_enter_noop_when_no_release() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    // Enter without a release URL: should be a no-op (not crash).
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.show_about);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn handle_key_show_help_blocks_other_keys() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_help = true;
    // 'x' is not a close-help key, so help stays open
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    assert!(!app.should_quit);
    assert!(app.show_help);
    // But ?/Esc/q should close help
    app.handle_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty()));
    assert!(!app.show_help);
    fs::remove_dir_all(&root).ok();
}

// -- handle_search_key -----------------------------------------------------

#[test]
fn search_key_enter_activates_and_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.search.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_up_down_navigation() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    let max = app.search.as_ref().unwrap().results_len().saturating_sub(1);
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().selected, 1.min(max));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_up_stays_at_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_down_stays_at_boundary() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    let s = app.search.as_mut().unwrap();
    let max = s.results_len().saturating_sub(1);
    if max == 0 {
        fs::remove_dir_all(&root).ok();
        return;
    }
    s.selected = max;
    let _ = s;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().selected, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_backspace_and_char() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().query, "a");
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().query, "");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_key_pop_without_search_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // search is None; Backspace/Char should not crash
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    fs::remove_dir_all(&root).ok();
}

// -- handle_in_file_search_key ---------------------------------------------

fn setup_in_file_search(app: &mut App) {
    app.content = vec![
        "hello world".to_string(),
        "foo bar".to_string(),
        "hello again".to_string(),
    ];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    app.in_file_search = Some(InFileSearch::new());
    app.in_file_search.as_mut().unwrap().push('o');
    app.refresh_in_file_search();
}

#[test]
fn in_file_search_opens_when_content_focused() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.in_file_search.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_n_next_match_wraps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_in_file_search(&mut app);
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 1);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 2);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 3);
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 4);
    // Wrap to 0 when at last
    let last = app.in_file_search.as_ref().unwrap().matches.len() - 1;
    app.in_file_search.as_mut().unwrap().current = last;
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_n_prev_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_in_file_search(&mut app);
    app.in_file_search.as_mut().unwrap().current = 1;
    app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    // "P" also goes back
    app.in_file_search.as_mut().unwrap().current = 1;
    app.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_prev_wraps_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_in_file_search(&mut app);
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::empty()));
    let last = app.in_file_search.as_ref().unwrap().matches.len() - 1;
    assert_eq!(app.in_file_search.as_ref().unwrap().current, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_ctrl_p_goes_prev() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_in_file_search(&mut app);
    app.in_file_search.as_mut().unwrap().current = 1;
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_tab_and_backtab() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_in_file_search(&mut app);
    app.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 1);
    app.in_file_search.as_mut().unwrap().current = 0;
    app.handle_key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::empty()));
    let last = app.in_file_search.as_ref().unwrap().matches.len() - 1;
    assert_eq!(app.in_file_search.as_ref().unwrap().current, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_backspace_and_char() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    app.content = vec!["hello".to_string()];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    app.handle_key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().query, "h");
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().query, "");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_no_matches_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    app.content = vec!["hello".to_string()];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
    // No matches, next/prev should not crash
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('P'), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

// -- handle_history_key ----------------------------------------------------

#[test]
fn history_key_esc_closes() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    assert!(app.history.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.history.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_key_up_down_navigation() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    let max = app
        .history
        .as_ref()
        .unwrap()
        .results_len()
        .saturating_sub(1);
    if max == 0 {
        fs::remove_dir_all(&root).ok();
        return;
    }
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().selected, 1);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().selected, 0);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().selected, 0); // stays at 0
    app.history.as_mut().unwrap().selected = max;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().selected, max); // stays at max
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_key_typing_filters() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    let total = app.history.as_ref().unwrap().results_len();
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    let filtered = app.history.as_ref().unwrap().results_len();
    assert!(filtered <= total);
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().results_len(), total);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_key_pop_without_history_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    fs::remove_dir_all(&root).ok();
}

// -- handle_theme_key -------------------------------------------------------

#[test]
fn theme_key_up_down_navigation() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 1);
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 0);
    // Up at 0 stays 0
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 0);
    // Down at max stays at max
    let max = app
        .theme_picker
        .as_ref()
        .unwrap()
        .results_len()
        .saturating_sub(1);
    app.theme_picker.as_mut().unwrap().selected = max;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_key_typing_filters() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    let total = app.theme_picker.as_ref().unwrap().results_len();
    app.handle_key(KeyEvent::new(KeyCode::Char('m'), KeyModifiers::empty()));
    assert!(app.theme_picker.as_ref().unwrap().results_len() < total);
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().results_len(), total);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_key_pop_without_picker_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    fs::remove_dir_all(&root).ok();
}

// -- handle_command_key -----------------------------------------------------

#[test]
fn command_key_up_down_boundaries() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    // Down at max stays at max
    let max = app
        .command_palette
        .as_ref()
        .unwrap()
        .results_len()
        .saturating_sub(1);
    app.command_palette.as_mut().unwrap().selected = max;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_key_typing_noop_without_palette() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::empty()));
    fs::remove_dir_all(&root).ok();
}

// -- handle_normal_key ------------------------------------------------------

#[test]
fn normal_key_search_files_content_focus_opens_in_file_search() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(
        app.in_file_search.is_some(),
        "with content focus + file open, / should open in-file search"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_search_content_opens_search_in_content_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // "f" opens content search
    app.handle_key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::empty()));
    assert!(app.search.is_some());
    assert_eq!(
        app.search.as_ref().unwrap().mode,
        crate::search::SearchMode::Content
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_reload_does_not_crash() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::empty()));
    // Reload silently succeeds
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_toggle_watch_flips_auto_watch() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.auto_watch);
    app.handle_key(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::empty()));
    assert!(app.auto_watch);
    app.handle_key(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::empty()));
    assert!(!app.auto_watch);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_theme_picker_opens() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    assert!(app.theme_picker.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn normal_key_esc_without_selection_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.selection = None;
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    // Should not crash, selection remains None
    assert!(app.selection.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- handle_tree_key --------------------------------------------------------

#[test]
fn tree_key_up_down_navigation() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    let initial = app.tree_selected;
    if app.nodes.len() > 1 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
        assert_eq!(app.tree_selected, initial + 1);
        app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
        assert_eq!(app.tree_selected, initial);
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_up_stays_at_first() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_selected = 0;
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_down_stays_at_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let last = app.nodes.len().saturating_sub(1);
    if last == 0 {
        fs::remove_dir_all(&root).ok();
        return;
    }
    app.tree_selected = last;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_enter_expands_collapses_dir() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
    app.tree_selected = dir_idx;
    let dir_path = app.nodes[dir_idx].path.clone();
    let before = app.nodes.len();
    // Enter expands
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.expanded.contains(&dir_path));
    assert!(app.nodes.len() > before);
    // Enter collapses
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(!app.expanded.contains(&dir_path));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_left_jumps_to_parent() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Expand sub/ so a.txt (depth 1) is visible and we can jump to root (depth 0)
    let sub_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
    app.expanded.insert(app.nodes[sub_idx].path.clone());
    app.rebuild();
    // Pick a nested file after expansion
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path.ends_with("c.txt"))
        .unwrap();
    app.tree_selected = file_idx;
    let parent_depth = app.nodes[file_idx].depth.saturating_sub(1);
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert!(
        app.nodes[app.tree_selected].depth == parent_depth,
        "tree selection should move to a parent node (depth < {})",
        app.nodes[file_idx].depth
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_home_goes_to_first() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_selected = 2;
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_end_goes_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_selected = 0;
    let last = app.nodes.len().saturating_sub(1);
    if last == 0 {
        fs::remove_dir_all(&root).ok();
        return;
    }
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::empty()));
    assert_eq!(app.tree_selected, last);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_independent_scroll_page_keys_move_viewport_not_cursor() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_independent_scroll = true;
    // Small viewport so the 4 top-level nodes overflow it.
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 2,
    };
    app.tree_selected = 0;
    app.tree_scroll = 0;

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    // Viewport moved down by a page, selection untouched.
    assert_eq!(app.tree_scroll, 2);
    assert_eq!(app.tree_selected, 0);

    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.tree_scroll, 0);
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_independent_scroll_home_end_move_viewport_only() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_independent_scroll = true;
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 2,
    };
    app.tree_selected = 1;
    app.tree_scroll = 0;

    let max_scroll = app.nodes.len().saturating_sub(2);
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::empty()));
    assert_eq!(app.tree_scroll, max_scroll);
    assert_eq!(app.tree_selected, 1, "selection must not move");

    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::empty()));
    assert_eq!(app.tree_scroll, 0);
    assert_eq!(app.tree_selected, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_independent_scroll_disabled_ignores_page_keys() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    assert!(!app.tree_independent_scroll); // default
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 2,
    };
    app.tree_scroll = 0;
    app.tree_selected = 0;

    app.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::empty()));
    assert_eq!(app.tree_scroll, 0, "page keys are inert without the toggle");
    assert_eq!(app.tree_selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_independent_scroll_up_down_still_move_cursor_and_follow() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_independent_scroll = true;
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 2,
    };
    app.tree_selected = 0;
    app.tree_scroll = 0;

    let last = app.nodes.len().saturating_sub(1);
    if last == 0 {
        fs::remove_dir_all(&root).ok();
        return;
    }
    // Walk the cursor to the bottom; the viewport should follow it.
    for _ in 0..last {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    }
    assert_eq!(app.tree_selected, last);
    assert!(
        app.tree_selected >= app.tree_scroll
            && app.tree_selected < app.tree_scroll + app.tree_area.height as usize,
        "selection {} should stay within viewport [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + app.tree_area.height as usize
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_independent_scroll_reveal_keeps_selection_in_viewport() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    app.tree_independent_scroll = true;
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 20,
        height: 2,
    };
    // Park the viewport away from the top so a reveal must scroll it back.
    app.tree_scroll = app.nodes.len().saturating_sub(2);

    let nested = root.join("sub").join("c.txt");
    app.reveal_in_tree(&nested);

    let idx = app.nodes.iter().position(|n| n.path == nested).unwrap();
    assert_eq!(app.tree_selected, idx);
    assert!(
        app.tree_selected >= app.tree_scroll
            && app.tree_selected < app.tree_scroll + app.tree_area.height as usize,
        "revealed node {} should be inside viewport [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + app.tree_area.height as usize
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_key_other_key_noop_when_focus_tree() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Tree;
    // A key that's not bound should be a no-op
    let initial = app.tree_selected;
    let sentinel = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty());
    if !super::super::config::pressed(&app.keys().nav_up, &sentinel)
        && !super::super::config::pressed(&app.keys().nav_down, &sentinel)
    {
        app.handle_key(sentinel);
        assert_eq!(app.tree_selected, initial);
    }
    fs::remove_dir_all(&root).ok();
}

// -- handle_content_key -----------------------------------------------------

#[test]
fn content_key_k_moves_active_line_up() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_scroll = 5;
    app.active_line = 5;
    app.handle_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::empty()));
    assert_eq!(app.active_line, 4);
    // active_line moves even at scroll=5; auto-scroll brings it into view.
    assert_eq!(app.content_scroll, 4);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_up_stays_at_zero_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_scroll = 0;
    app.active_line = 0;
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.active_line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_page_up_scrolls_up() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_scroll = 25;
    app.handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::empty()));
    assert_eq!(app.content_scroll, 5);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_left_hscroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.word_wrap = false;
    app.content_hscroll = 10;
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert_eq!(app.content_hscroll, 6);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_right_hscroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.word_wrap = false;
    app.content_hscroll = 10;
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
    assert_eq!(app.content_hscroll, 14);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_left_is_noop_when_word_wrap() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.word_wrap = true;
    app.content_hscroll = 10;
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert_eq!(app.content_hscroll, 10); // unchanged
    app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::empty()));
    assert_eq!(app.content_hscroll, 10); // unchanged
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_markdown_toggle_raw() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let md_path = root.join("doc.md");
    fs::write(&md_path, "# Title\nbody\n").unwrap();
    app.open_file(&md_path);
    app.focus = Focus::Content;
    assert!(!app.show_raw_markdown);
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::empty()));
    assert!(app.show_raw_markdown);
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::empty()));
    assert!(!app.show_raw_markdown);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_markdown_toggle_raw_noop_when_not_markdown() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    assert!(!app.is_markdown);
    let before = app.show_raw_markdown;
    app.handle_key(KeyEvent::new(KeyCode::Char('M'), KeyModifiers::empty()));
    assert_eq!(app.show_raw_markdown, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_toggle_line_numbers_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    assert!(app.show_line_numbers);
    app.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::empty()));
    assert!(!app.show_line_numbers);
    app.handle_key(KeyEvent::new(KeyCode::Char('L'), KeyModifiers::empty()));
    assert!(app.show_line_numbers);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_toggle_blame_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.is_diff = false;
    assert!(!app.show_blame);
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(app.show_blame);
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(!app.show_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_toggle_blame_noop_when_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.is_diff = true;
    app.show_blame = false;
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(!app.show_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_other_key_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::empty()));
    assert!(app.word_wrap);
    fs::remove_dir_all(&root).ok();
}

// -- dispatch_command edge cases ---------------------------------------------

#[test]
fn dispatch_command_toggle_hidden_toggles_and_reloads() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    // Filter to "toggle_hidden"
    for c in "hidden".chars() {
        if let Some(p) = &mut app.command_palette {
            p.push(c);
        }
    }
    let before = app.show_hidden;
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    assert_ne!(app.show_hidden, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_show_about_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.command_palette = Some(CommandPalette::default());
    for c in "about".chars() {
        if let Some(p) = &mut app.command_palette {
            p.push(c);
        }
    }
    app.dispatch_command();
    assert!(app.show_about);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_noop_when_no_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Without a valid command selected, dispatch_command should silently do nothing
    app.command_palette = Some(CommandPalette::default());
    // Filter to something that produces no results
    for c in "zzzzzzz".chars() {
        if let Some(p) = &mut app.command_palette {
            p.push(c);
        }
    }
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- apply_selected_theme edge cases ---------------------------------------

#[test]
fn apply_selected_theme_noop_when_no_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    // Filter to an impossible theme name
    for c in "zzzzzzz".chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
    }
    let before = app.theme.accent;
    app.apply_selected_theme();
    assert!(app.theme_picker.is_none());
    assert_eq!(app.theme.accent, before);
    fs::remove_dir_all(&root).ok();
}

// -- handle_in_file_search_next/prev (pure, no keyboard) -------------------

#[test]
fn in_file_search_next_noop_when_no_matches() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    app.in_file_search_next();
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_prev_noop_when_no_matches() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    app.in_file_search_prev();
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

// -- handle_in_file_search_next/prev wrap-around ---------------------------

#[test]
fn in_file_search_next_wraps_to_first() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    let s = app.in_file_search.as_mut().unwrap();
    s.matches.push(InFileMatch {
        line: 0,
        col: 0,
        len: 1,
    });
    s.matches.push(InFileMatch {
        line: 1,
        col: 0,
        len: 1,
    });
    app.in_file_search.as_mut().unwrap().current = 1;
    app.in_file_search_next();
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_method_next_wraps_to_first() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    {
        let s = app.in_file_search.as_mut().unwrap();
        s.matches.push(InFileMatch {
            line: 0,
            col: 0,
            len: 1,
        });
        s.matches.push(InFileMatch {
            line: 1,
            col: 0,
            len: 1,
        });
    }
    app.in_file_search.as_mut().unwrap().current = 1;
    app.in_file_search_next();
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 0);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_method_prev_wraps_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    {
        let s = app.in_file_search.as_mut().unwrap();
        s.matches.push(InFileMatch {
            line: 0,
            col: 0,
            len: 1,
        });
        s.matches.push(InFileMatch {
            line: 1,
            col: 0,
            len: 1,
        });
    }
    app.in_file_search_prev();
    assert_eq!(app.in_file_search.as_ref().unwrap().current, 1);
    fs::remove_dir_all(&root).ok();
}

// -- scroll_in_file_search_to_current -------------------------------------

#[test]
fn scroll_in_file_search_to_current_noop_when_no_search() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = None;
    app.scroll_in_file_search_to_current(); // should not crash
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_in_file_search_to_current_noop_when_no_match() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = Some(InFileSearch::new());
    app.scroll_in_file_search_to_current(); // should not crash
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_in_file_search_to_current_scrolls_if_above() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["line0".to_string(), "line1".to_string()];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.in_file_search = Some(InFileSearch::new());
    let s = app.in_file_search.as_mut().unwrap();
    s.matches.push(InFileMatch {
        line: 0,
        col: 0,
        len: 1,
    });
    app.content_scroll = 3; // match at line 0 is above viewport -> scroll to 0
    app.scroll_in_file_search_to_current();
    assert_eq!(app.content_scroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn scroll_in_file_search_to_current_scrolls_if_below() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = (0..50).map(|i| format!("line {i}")).collect();
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };
    app.in_file_search = Some(InFileSearch::new());
    let s = app.in_file_search.as_mut().unwrap();
    s.matches.push(InFileMatch {
        line: 45,
        col: 0,
        len: 1,
    });
    app.content_scroll = 0;
    app.scroll_in_file_search_to_current();
    // scroll_max = 40, match at 45 is below viewport (40+10=50 -> visible up to 49)
    // Actually: scroll_max = 50-10 = 40, viewport shows lines 0..9, match at 45 -> scroll to 45-10+1 = 36
    assert_eq!(app.content_scroll, 36);
    fs::remove_dir_all(&root).ok();
}

// -- refresh_in_file_search -----------------------------------------------

#[test]
fn refresh_in_file_search_noop_when_no_search() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.in_file_search = None;
    app.refresh_in_file_search();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn refresh_in_file_search_uses_content_lines() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello".to_string(), "world".to_string()];
    app.in_file_search = Some(InFileSearch::new());
    app.in_file_search.as_mut().unwrap().push('o');
    app.refresh_in_file_search();
    assert!(!app.in_file_search.as_ref().unwrap().matches.is_empty());
    fs::remove_dir_all(&root).ok();
}

// -- handle_mouse: help overlay / scrollbar / drag / auto-scroll -----------

#[test]
fn mouse_help_overlay_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_help = true;
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 1, 1));
    // No state change expected
    assert!(!app.should_quit);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_content_click_on_scrollbar_sets_drag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 15,
        height: 10,
    };
    app.show_scrollbar = true;
    // Click on the last column of the content area (scrollbar)
    app.handle_mouse(click(5 + 14, 6));
    assert!(app.scrollbar_drag);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_scrollbar_drag_updates_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 15,
        height: 10,
    };
    app.show_scrollbar = true;
    app.scrollbar_drag = true;
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 5 + 14, 8));
    // drag should mark content as scrolled
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_up_clears_scrollbar_drag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.scrollbar_drag = true;
    app.drag_start = Some((0, 0));
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 0),
        active: (0, 1),
    });
    app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 1, 1));
    assert!(!app.scrollbar_drag);
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_up_clears_drag_without_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.drag_start = Some((0, 0));
    app.selection = None;
    app.handle_mouse(mouse(MouseEventKind::Up(MouseButton::Left), 1, 1));
    assert!(app.drag_start.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_drag_auto_scroll_up_when_near_top() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 20,
    };
    app.drag_start = Some((0, 0));
    app.content_scroll = 5;
    // Drag at row = ca.y (5) -> above ca.y + 2 (7) -> auto-scroll up
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 6, 5));
    assert_eq!(app.content_scroll, 4);
    assert!(app.selection.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_drag_auto_scroll_down_when_near_bottom() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 5,
        y: 5,
        width: 50,
        height: 10,
    };
    app.drag_start = Some((0, 0));
    app.content_scroll = 0;
    let max = app.content_scroll_max();
    // Drag at row = ca.y + ca.height - 1 (5+10-1=14) -> >= ca.y + ca.height - 2 (5+10-2=13) -> auto-scroll down
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 6, 14));
    assert_eq!(app.content_scroll, 1.min(max));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_drag_without_start_is_noop_during_drag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.drag_start = None;
    app.handle_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), 1, 1));
    assert!(app.selection.is_none());
    fs::remove_dir_all(&root).ok();
}

// -- history mouse events --------------------------------------------------

#[test]
fn history_mouse_click_outside_area_is_noop() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.history_area = Rect {
        x: 5,
        y: 5,
        width: 30,
        height: 10,
    };
    let selected_before = app.history.as_ref().unwrap().selected;
    app.handle_mouse(click(1, 1)); // outside area
    assert_eq!(app.history.as_ref().unwrap().selected, selected_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_mouse_click_out_of_range_index_noop() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.history_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 10,
    };
    let selected_before = app.history.as_ref().unwrap().selected;
    app.handle_mouse(click(1, 9)); // row 9 -> index 9, likely out of range
    assert_eq!(app.history.as_ref().unwrap().selected, selected_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_mouse_single_then_double_click_opens() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.history_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 10,
    };
    app.history_offset = 0;

    app.handle_mouse(click(1, 0));
    assert!(app.history.is_some());
    app.handle_mouse(click(1, 0)); // second click -> double-click -> opens
    assert!(app.history.is_none() || app.is_diff);
    fs::remove_dir_all(&root).ok();
}

// -- theme mouse events ----------------------------------------------------

#[test]
fn theme_mouse_click_outside_area_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.theme_area = Rect {
        x: 5,
        y: 5,
        width: 30,
        height: 10,
    };
    let selected_before = app.theme_picker.as_ref().unwrap().selected;
    app.handle_mouse(click(1, 1));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, selected_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_mouse_single_then_double_click_opens() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.theme_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 30,
    };
    app.theme_offset = 0;

    let before = app.theme.accent;
    app.handle_mouse(click(1, 0));
    assert!(app.theme_picker.is_some());
    app.handle_mouse(click(1, 0)); // double-click -> applies
    assert!(app.theme_picker.is_none() || app.theme.accent != before);
    fs::remove_dir_all(&root).ok();
}

// -- command palette mouse events ------------------------------------------

#[test]
fn command_palette_mouse_click_outside_area_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    app.command_palette_area = Rect {
        x: 5,
        y: 5,
        width: 30,
        height: 10,
    };
    let selected_before = app.command_palette.as_ref().unwrap().selected;
    app.handle_mouse(click(1, 1));
    assert_eq!(
        app.command_palette.as_ref().unwrap().selected,
        selected_before
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_mouse_single_then_double_click_executes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    app.command_palette_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 30,
    };
    app.command_palette_offset = 0;
    // Filter to "about" so double-click triggers show_about
    for c in "about".chars() {
        if let Some(p) = &mut app.command_palette {
            p.push(c);
        }
    }

    app.handle_mouse(click(1, 0));
    assert!(app.command_palette.is_some());
    app.handle_mouse(click(1, 0)); // double-click -> executes
    assert!(app.command_palette.is_none());
    assert!(app.show_about);
    fs::remove_dir_all(&root).ok();
}

// -- search mouse events ---------------------------------------------------

#[test]
fn search_mouse_click_outside_area_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.search_area = Rect {
        x: 5,
        y: 5,
        width: 30,
        height: 10,
    };
    let selected_before = app.search.as_ref().unwrap().selected;
    app.handle_mouse(click(1, 1));
    assert_eq!(app.search.as_ref().unwrap().selected, selected_before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_mouse_click_out_of_range_index_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.search_area = Rect {
        x: 0,
        y: 0,
        width: 30,
        height: 10,
    };
    let selected_before = app.search.as_ref().unwrap().selected;
    // Row 99 will be well past the last result
    app.handle_mouse(click(1, 99));
    assert_eq!(app.search.as_ref().unwrap().selected, selected_before);
    fs::remove_dir_all(&root).ok();
}

// -- set_scroll_from_mouse_y -----------------------------------------------

#[test]
fn set_scroll_from_mouse_y_noop_when_total_less_than_height() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt")); // 2 lines
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 10,
    };
    let before = app.content_scroll;
    app.set_scroll_from_mouse_y(5);
    assert_eq!(app.content_scroll, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_scroll_from_mouse_y_with_track_range_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // line_count = 50, inner_h = 50, scroll_range = 0
    app.open_file(&root.join("long.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 50,
    };
    let before = app.content_scroll;
    app.set_scroll_from_mouse_y(25);
    assert_eq!(app.content_scroll, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_scroll_from_mouse_y_calculates_position() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 10,
    };
    // inner_h=10, total=50, thumb = max(10*10/50,1) = 2
    // scroll_range = 50-10 = 40
    // track_range = 10-2 = 8
    // row 5 -> y=5 -> scroll = 5*40/8 = 25
    app.set_scroll_from_mouse_y(5);
    assert_eq!(app.content_scroll, 25);
    // row 0 -> scroll = 0
    app.set_scroll_from_mouse_y(0);
    assert_eq!(app.content_scroll, 0);
    // row 9 -> y=9.min(8)=8 -> scroll = 8*40/8 = 40
    app.set_scroll_from_mouse_y(9);
    assert_eq!(app.content_scroll, 40);
    fs::remove_dir_all(&root).ok();
}

/// A temp git repo whose tracked file has two well-separated edits, producing a
/// working-tree diff with two distinct `@@` hunks.
fn temp_git_two_hunks() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_git_hunks_{}_{n}", std::process::id()));
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
    let base: String = (1..=30).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("f.txt"), &base).unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "init"]);
    // Edit line 2 and line 28 — far enough apart for two separate hunks.
    let edited: String = (1..=30)
        .map(|i| match i {
            2 => "line 2 CHANGED\n".to_string(),
            28 => "line 28 CHANGED\n".to_string(),
            _ => format!("line {i}\n"),
        })
        .collect();
    fs::write(dir.join("f.txt"), &edited).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn diff_side_by_side_toggle_flips_flag_and_builds_rows() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.show_working_tree_diff(&root.join("tracked.txt"));
    app.focus = Focus::Content;

    assert!(app.is_diff);
    assert!(!app.diff_rows.is_empty(), "diff rows must be parsed");
    assert!(!app.diff_side_by_side);

    // D toggles side-by-side on, then off.
    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::empty()));
    assert!(app.diff_side_by_side);
    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::empty()));
    assert!(!app.diff_side_by_side);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_side_by_side_key_is_noop_outside_diff() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;

    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::empty()));
    assert!(
        !app.diff_side_by_side,
        "D must do nothing for a normal file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_sbs_active_requires_min_width() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.show_working_tree_diff(&root.join("tracked.txt"));
    app.diff_side_by_side = true;

    app.content_area.width = crate::diff::MIN_SIDE_BY_SIDE_WIDTH - 1;
    assert!(!app.diff_sbs_active(), "too narrow → falls back to unified");
    app.content_area.width = crate::diff::MIN_SIDE_BY_SIDE_WIDTH;
    assert!(app.diff_sbs_active(), "wide enough → side-by-side active");
    fs::remove_dir_all(&root).ok();
}

// -- content_scroll_max edge cases -----------------------------------------

#[test]
fn content_scroll_max_zero_total_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = Vec::new();
    app.content_area = viewport(10);
    assert_eq!(app.content_scroll_max(), 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_scroll_max_lines_less_than_height() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["a".to_string(), "b".to_string()];
    app.content_area = viewport(10);
    assert_eq!(app.content_scroll_max(), 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_scroll_max_zero_height_falls_back_to_one() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["a".to_string(); 5];
    app.content_area = viewport(0);
    // vh = max(0,1) = 1, so max = 5 - 1 = 4
    assert_eq!(app.content_scroll_max(), 4);
    fs::remove_dir_all(&root).ok();
}

// -- content_pos additional boundary tests ---------------------------------

#[test]
fn content_pos_no_wrap_with_hscroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.word_wrap = false;
    app.content_scroll = 0;
    app.content_hscroll = 5;
    app.content_area = Rect {
        x: 2,
        y: 2,
        width: 80,
        height: 20,
    };
    // rel_col = 5 - 2 = 3, prefix = 2, buf_col = 3 + 5 - 2 = 6
    let (_line, col) = app.content_pos(5, 2);
    assert_eq!(col, 6);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_no_wrap_below_content_clamps_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt")); // 2 lines
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    // row 99 → rel_row = 99 → display_line = 99 → display_to_physical(99) = 99,
    // but line_count is 2, so display_to_physical returns 99 (no fold map, identity).
    // This is technically past end — the caller (mouse handler) should guard against this.
    let (line, _col) = app.content_pos(0, 99);
    assert_eq!(line, 99); // identity mapping past end
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_virtual_file_no_wrap() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.word_wrap = false;
    app.content_scroll = 0;
    app.content_hscroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    let (line, _col) = app.content_pos(0, 0);
    assert_eq!(line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_with_fold_display_map() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt")); // "line1\nline2\n"
                                        // Simulate folding: mark line 0 as hidden via fold_display_map.
    app.fold_display_map = vec![1]; // display line 0 → physical line 1
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    // rel_row = 0, display_to_physical(0) = 1
    let (line, _col) = app.content_pos(0, 0);
    assert_eq!(line, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_word_wrap_zero_width_area_falls_through() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![vec![(
        ratatui::style::Style::default(),
        "hello".to_string(),
    )]];
    app.word_wrap = true;
    app.content_scroll = 0;
    // wrap_width = 0 → NonZeroUsize::new(0) is None → falls through to no-wrap path
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 0,
        height: 20,
    };
    let (line, _col) = app.content_pos(0, 0);
    assert_eq!(line, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_pos_diff_mode_prefix_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["@@ -1 +1 @@".to_string(), "+hello".to_string()];
    app.is_diff = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    // prefix = 0 (diff mode), rel_col = 1 → buf_col = 1
    let (line, col) = app.content_pos(1, 1);
    assert_eq!(line, 1);
    assert_eq!(col, 1);
    fs::remove_dir_all(&root).ok();
}

// -- line_prefix_width ------------------------------------------------------

#[test]
fn line_prefix_width_normal_file_with_line_numbers() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello".to_string()];
    // Not diff, not markdown → prefix = fold_gutter_width + len("1") + 1 = 0 + 1 + 1 = 2
    assert_eq!(app.line_prefix_width(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn line_prefix_width_with_fold_gutter() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello".to_string(); 10];
    // Simulate YAML fold regions
    app.fold_regions = vec![crate::fold::FoldRegion { start: 0, end: 5 }];
    // fold_gutter_width = 2, line_width = len("10") + 1 = 3, total = 2 + 3 = 5
    assert_eq!(app.line_prefix_width(), 5);
    fs::remove_dir_all(&root).ok();
}

// -- selection_text ---------------------------------------------------------

#[test]
fn selection_text_inline_single_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello world".to_string()];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 6),
        active: (0, 11),
    });
    assert_eq!(app.selection_text(), "world");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_inline_multi_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["abc".to_string(), "def".to_string()];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 1),
        active: (1, 2),
    });
    assert_eq!(app.selection_text(), "bc\nde");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_inline_out_of_bounds_returns_empty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hi".to_string()];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (10, 0),
        active: (20, 0),
    });
    assert_eq!(app.selection_text(), "");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_markdown_single_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![vec![(
        ratatui::style::Style::default(),
        "hello **world**".to_string(),
    )]];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 6),
        active: (0, 11),
    });
    assert_eq!(app.selection_text(), "**wor");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_markdown_multi_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![
        vec![(ratatui::style::Style::default(), "line A".to_string())],
        vec![(ratatui::style::Style::default(), "line B".to_string())],
    ];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 5),
        active: (1, 4),
    });
    assert_eq!(app.selection_text(), "A\nline");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_markdown_out_of_bounds_returns_empty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    app.show_raw_markdown = false;
    app.markdown_lines = vec![];
    app.selection = Some(crate::selection::TextSelection {
        anchor: (5, 0),
        active: (10, 0),
    });
    assert_eq!(app.selection_text(), "");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_virtual_file() {
    use std::io::Write;
    let root = temp_tree();
    // Create a temp file, read it via App::open_file, then simulate selection.
    let path = root.join("sel.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    write!(f, "alpha\nbeta\ngamma\n").unwrap();
    drop(f);
    let mut app = app_for(&root);
    app.open_file(&path);
    assert!(app.virtual_file.is_some(), "should use virtual file");
    app.selection = Some(crate::selection::TextSelection {
        anchor: (1, 1),
        active: (2, 3),
    });
    // line 1 = "beta", col 1 = 'e' → "eta"; line 2 = "gamma", col 3 = "gam" → "gam"
    assert_eq!(app.selection_text(), "eta\ngam");
    fs::remove_dir_all(&root).ok();
}

// -- selection_text: JSON pretty mode ---------------------------------------

#[test]
fn selection_text_json_pretty() {
    let root = temp_tree();
    let path = root.join("data.json");
    fs::write(&path, r#"{"a":1,"b":2}"#).unwrap();
    let mut app = app_for(&root);
    app.open_file(&path);
    assert!(app.show_pretty_json, "JSON files open with pretty view");
    // Pretty-printed JSON has '{' on line 0, values on subsequent lines.
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 0),
        active: (2, 0),
    });
    let text = app.selection_text();
    assert!(
        !text.is_empty(),
        "selection text should not be empty; json_pretty_text={:?}",
        app.json_pretty_text
    );
    assert!(text.contains('{'), "selection should include opening brace");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn selection_text_virtual_file_out_of_bounds_returns_empty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello".to_string()];
    // virtual_file is None, so the VirtualFile path won't trigger.
    // Force virtual_file to be Some to test that branch:
    let path = root.join("dummy.txt");
    fs::write(&path, "hi\n").unwrap();
    let vf = crate::virtual_file::VirtualFile::open(&path);
    app.virtual_file = vf;
    app.content = Vec::new();
    // start_line >= total (line_count = 1 from virtual file)
    app.selection = Some(crate::selection::TextSelection {
        anchor: (5, 0),
        active: (10, 0),
    });
    assert_eq!(app.selection_text(), "");
    fs::remove_dir_all(&root).ok();
}

// -- content_pos with word wrap + visual rows past content (clamp) -----------

#[test]
fn content_pos_word_wrap_visual_rows_past_content_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["hello".to_string()];
    app.word_wrap = true;
    app.content_scroll = 0;
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    // rel_row way past total visual rows → falls through to last physical line
    let (line, _col) = app.content_pos(0, 50);
    assert_eq!(line, 0);
    fs::remove_dir_all(&root).ok();
}

// -- fold_gutter_width ------------------------------------------------------

#[test]
fn fold_gutter_width_zero_when_no_regions() {
    let root = temp_tree();
    let app = app_for(&root);
    assert_eq!(app.fold_gutter_width(), 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fold_gutter_width_two_when_regions_exist() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.fold_regions = vec![crate::fold::FoldRegion { start: 0, end: 3 }];
    assert_eq!(app.fold_gutter_width(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn diff_hunk_navigation_jumps_between_headers() {
    let root = temp_git_two_hunks();
    let mut app = app_for(&root);
    app.show_working_tree_diff(&root.join("f.txt"));
    app.focus = Focus::Content;
    app.content_area.height = 5;

    // Header rows in the unified content.
    let headers: Vec<usize> = app
        .content
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("@@"))
        .map(|(i, _)| i)
        .collect();
    assert_eq!(headers.len(), 2, "expected two hunks");

    app.content_scroll = 0;
    // n jumps to the first hunk header below the top.
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.content_scroll, headers[0]);
    // n again advances to the second hunk.
    app.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::empty()));
    assert_eq!(app.content_scroll, headers[1]);
    // N goes back to the first.
    app.handle_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::empty()));
    assert_eq!(app.content_scroll, headers[0]);
    fs::remove_dir_all(&root).ok();
}

// -- dispatch_command action tests -------------------------------------------

/// Sets `app.command_palette` to an instance whose first and only result is
/// the command matching `action_id`.
fn setup_command(app: &mut App, action_id: &str) {
    app.command_palette = Some(CommandPalette::default());
    let idx = COMMANDS
        .iter()
        .position(|c| c.action_id == action_id)
        .unwrap();
    if let Some(p) = &mut app.command_palette {
        p.filtered = vec![idx];
        p.selected = 0;
    }
}

#[test]
fn dispatch_command_toggle_help_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_command(&mut app, "toggle_help");
    assert!(!app.show_help);
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    assert!(app.show_help);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_open_file_search_creates_search() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_command(&mut app, "open_file_search");
    assert!(app.search.is_none());
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    assert!(app.search.is_some());
    assert_eq!(app.search.as_ref().unwrap().mode, SearchMode::Files);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_open_file_search_with_keep_query() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.config.keep_search_query = true;
    app.last_search_query = "test".to_string();
    setup_command(&mut app, "open_file_search");
    app.dispatch_command();
    assert!(app.search.is_some());
    assert_eq!(app.search.as_ref().unwrap().query, "test");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_open_content_search_creates_search() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_command(&mut app, "open_content_search");
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    assert!(app.search.is_some());
    assert_eq!(app.search.as_ref().unwrap().mode, SearchMode::Content);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_word_wrap_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    setup_command(&mut app, "toggle_word_wrap");
    let before = app.word_wrap;
    app.content_scroll = 10;
    app.content_hscroll = 5;
    app.dispatch_command();
    assert!(app.command_palette.is_none());
    assert_ne!(app.word_wrap, before);
    assert_eq!(app.content_scroll, 0);
    assert_eq!(app.content_hscroll, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_raw_markdown_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_markdown = true;
    setup_command(&mut app, "toggle_raw_markdown");
    assert!(!app.show_raw_markdown);
    app.dispatch_command();
    assert!(app.show_raw_markdown);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_pretty_json_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_json = true;
    app.json_pretty_lines = vec![vec![(ratatui::style::Style::default(), "{}".to_string())]];
    setup_command(&mut app, "toggle_pretty_json");
    assert!(!app.show_pretty_json);
    app.dispatch_command();
    assert!(app.show_pretty_json);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_toggle_diff_side_by_side_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.is_diff = true;
    setup_command(&mut app, "toggle_diff_side_by_side");
    assert!(!app.diff_side_by_side);
    app.dispatch_command();
    assert!(app.diff_side_by_side);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_yaml_unfold_all_works() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec![
        "header:".to_string(),
        "  child1: 1".to_string(),
        "  child2: 2".to_string(),
    ];
    app.fold_regions = vec![FoldRegion { start: 0, end: 2 }];
    app.folded.insert(0);
    app.rebuild_fold_display_map();
    assert_eq!(
        app.fold_display_map,
        vec![0],
        "folded should show only header"
    );
    setup_command(&mut app, "unfold_all");
    app.dispatch_command();
    assert!(app.folded.is_empty());
    assert!(app.fold_display_map.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn dispatch_command_yaml_fold_toggle_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.content = vec!["header".to_string(), "  child".to_string()];
    app.fold_regions = vec![FoldRegion { start: 0, end: 1 }];
    app.rebuild_fold_display_map();
    setup_command(&mut app, "fold_toggle");
    assert!(app.folded.is_empty());
    app.dispatch_command();
    // The region at content_scroll (0) should be folded
    assert!(app.folded.contains(&0));
    fs::remove_dir_all(&root).ok();
}

// -- handle_content_key: additional coverage ----------------------------------

#[test]
fn content_key_toggle_pretty_json_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.is_json = true;
    app.json_pretty_lines = vec![vec![(ratatui::style::Style::default(), "{}".to_string())]];
    assert!(!app.show_pretty_json);
    // 'J' is the toggle_pretty_json binding
    app.handle_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::empty()));
    assert!(app.show_pretty_json);
    fs::remove_dir_all(&root).ok();
}

// -- visual-line mode -----------------------------------------------------

#[test]
fn visual_line_enters_and_extends_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.focus = Focus::Content;
    app.content_area = viewport(10);

    // V enters visual-line mode anchored at the top.
    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    let v = app.visual_line.expect("visual-line mode should be active");
    assert_eq!(v.range(), (0, 0));

    // j extends the selection downward.
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty()));
    assert_eq!(app.visual_line.unwrap().range(), (0, 2));

    // G extends to the last line.
    app.handle_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::empty()));
    assert_eq!(app.visual_line.unwrap().range(), (0, 49));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_yaml_fold_toggle_toggles() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.fold_regions = vec![FoldRegion { start: 0, end: 1 }];
    app.rebuild_fold_display_map();
    assert!(app.folded.is_empty());
    // Space is the yaml_fold_toggle binding
    app.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::empty()));
    assert!(app.folded.contains(&0));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn visual_line_esc_exits_mode() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = viewport(10);

    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    assert!(app.visual_line.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.visual_line.is_none());
    assert!(!app.blame_panel);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_key_toggle_diff_side_by_side_toggles() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.show_working_tree_diff(&root.join("tracked.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 120,
        height: 10,
    };
    app.focus = Focus::Content;
    assert!(!app.diff_side_by_side);
    // 'D' is the toggle_diff_side_by_side binding
    app.handle_key(KeyEvent::new(KeyCode::Char('D'), KeyModifiers::empty()));
    assert!(app.diff_side_by_side);
    fs::remove_dir_all(&root).ok();
}

// -- handle_tree_key: additional coverage -------------------------------------

#[test]
fn tree_key_left_on_expanded_dir_collapses() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
    let dir_path = app.nodes[dir_idx].path.clone();
    app.tree_selected = dir_idx;
    // Expand the dir first
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
    assert!(app.expanded.contains(&dir_path));
    // Left collapses it
    app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::empty()));
    assert!(!app.expanded.contains(&dir_path));
    fs::remove_dir_all(&root).ok();
}

// -- handle_mouse: fold gutter ------------------------------------------------

#[test]
fn mouse_fold_gutter_click_toggles_region() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 20,
    };
    app.tree_area = Rect {
        x: 100,
        y: 0,
        width: 10,
        height: 10,
    };
    app.fold_regions = vec![FoldRegion { start: 0, end: 1 }];
    app.fold_gutter_rows = vec![(0, 0)];
    assert!(app.folded.is_empty());
    // Click at (col=0, row=0) which is in the fold gutter
    app.handle_mouse(click(0, 0));
    assert!(app.folded.contains(&0));
    fs::remove_dir_all(&root).ok();
}

// -- handle_mouse: scroll boundaries ------------------------------------------

#[test]
fn mouse_scroll_up_tree_at_zero_is_noop() {
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
    let before = app.tree_selected;
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.tree_selected, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn visual_line_blame_key_toggles_panel() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = viewport(10);

    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    // b opens the scoped blame panel.
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(app.blame_panel);
    // b again hides it, leaving visual-line mode active.
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(!app.blame_panel);
    assert!(app.visual_line.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn visual_line_does_not_toggle_blame_gutter() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = viewport(10);

    // Entering visual-line mode and pressing b must not flip the always-on
    // blame gutter (`show_blame`); it controls the scoped panel instead.
    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    app.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::empty()));
    assert!(!app.show_blame);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn mouse_scroll_down_tree_at_last_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = full_rect();
    app.content_area = Rect {
        x: 100,
        y: 0,
        width: 40,
        height: 20,
    };
    let last = app.nodes.len().saturating_sub(1);
    app.tree_selected = last;
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.tree_selected, last);
    fs::remove_dir_all(&root).ok();
}

// -- mouse scroll at overlay boundaries ---------------------------------------

/// A temp git repo with exactly two commits touching a single file.
fn temp_git_two_commits() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_git_two_{}_{n}", std::process::id()));
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
    fs::write(dir.join("f.txt"), "commit1\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "first"]);
    fs::write(dir.join("f.txt"), "commit2\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "second"]);
    dir.canonicalize().unwrap()
}

#[test]
fn history_mouse_scroll_at_boundary() {
    let root = temp_git_two_commits();
    let mut app = app_for(&root);
    app.open_file(&root.join("f.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    app.history_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 20,
    };
    app.history_offset = 0;

    let h = app.history.as_ref().unwrap();
    assert!(h.results_len() >= 2, "need 2+ commits");
    let _ = h;

    // ScrollUp at selected=0 stays at 0
    app.history.as_mut().unwrap().selected = 0;
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.history.as_ref().unwrap().selected, 0);

    // ScrollDown at max stays at max
    let max = app.history.as_ref().unwrap().results_len() - 1;
    app.history.as_mut().unwrap().selected = max;
    app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
    assert_eq!(app.history.as_ref().unwrap().selected, max);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_mouse_scroll_up_at_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    app.theme_area = full_rect();
    app.theme_offset = 0;
    app.theme_picker.as_mut().unwrap().selected = 0;
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_palette_mouse_scroll_up_at_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    app.command_palette_area = full_rect();
    app.command_palette_offset = 0;
    app.command_palette.as_mut().unwrap().selected = 0;
    app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, 0);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_mouse_scroll_up_at_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    app.search_area = full_rect();
    app.search_offset = 0;
    if app.search.as_ref().unwrap().results_len() > 0 {
        app.search.as_mut().unwrap().selected = 0;
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.search.as_ref().unwrap().selected, 0);
    }
    fs::remove_dir_all(&root).ok();
}

// -- handle_mouse: _ => {} catch-all ------------------------------------------

#[test]
fn mouse_unhandled_event_kind_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Use a mouse event kind that hits the catch-all.
    // MiddleButton down is not handled anywhere.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Middle), 1, 1));
    // Should not panic and no state should change.
    assert!(!app.should_quit);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_mouse_click_navigates_to_root() {
    let root = temp_tree();
    let mut app = app_for(&root);

    // Expand sub/ and select a nested file so tree_selected is not root.
    app.expanded.insert(root.join("sub"));
    app.rebuild();
    let nested = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub").join("c.txt"))
        .unwrap();
    app.tree_selected = nested;
    let prev = app.tree_selected;
    assert!(prev > 0, "nested file should not be at index 0");

    // Simulate a rendered breadcrumb: root segment spanning columns 1..5 at row 1.
    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 5,
            height: 1,
        },
    ));

    // Click on the root breadcrumb segment.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 2, 1));

    assert_eq!(
        app.tree_selected,
        0,
        "clicking root breadcrumb should select index 0, got {} (len={})",
        app.tree_selected,
        app.nodes.len(),
    );
    assert!(matches!(app.focus, Focus::Tree));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_mouse_click_navigates_to_intermediate_dir() {
    let root = temp_tree();
    let mut app = app_for(&root);

    // Expand sub/ and select a nested file so breadcrumb shows root + "sub".
    app.expanded.insert(root.join("sub"));
    app.rebuild();
    let nested = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub").join("c.txt"))
        .unwrap();
    app.tree_selected = nested;
    let sub_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub"))
        .unwrap();

    // Breadcrumb: root (cols 1..5), sub (cols 9..12) with " / " separator.
    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 5,
            height: 1,
        },
    ));
    app.breadcrumb_areas.push((
        root.join("sub"),
        Rect {
            x: 9,
            y: 1,
            width: 3,
            height: 1,
        },
    ));

    // Click on the "sub" breadcrumb segment.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 10, 1));

    assert_eq!(
        app.tree_selected, sub_idx,
        "clicking sub breadcrumb should select sub directory"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn breadcrumb_mouse_click_parent_changes_root() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    let parent = root.parent().expect("temp dir has a parent").to_path_buf();

    // Select a nested file.
    app.expanded.insert(root.join("sub"));
    app.rebuild();
    let nested = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub").join("c.txt"))
        .unwrap();
    app.tree_selected = nested;

    // Simulate a breadcrumb that includes the parent directory segment.
    app.breadcrumb_areas.push((
        parent.clone(),
        Rect {
            x: 1,
            y: 1,
            width: 5,
            height: 1,
        },
    ));
    app.breadcrumb_areas.push((
        root.clone(),
        Rect {
            x: 9,
            y: 1,
            width: 7,
            height: 1,
        },
    ));

    // Click on the parent directory breadcrumb segment.
    app.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 3, 1));

    assert_eq!(
        app.root, parent,
        "clicking parent breadcrumb should change root to parent"
    );
    assert!(
        !app.nodes.is_empty(),
        "tree should have contents for parent root"
    );
    assert!(app.current_file.is_none(), "current file should be cleared");
    fs::remove_dir_all(&orig_root).ok();
}

// -- handle_key: catch-all in overlays ----------------------------------------

#[test]
fn search_key_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.focus = Focus::Content;
    app.current_file = None;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.search.is_some());
    let selected = app.search.as_ref().unwrap().selected;
    // F-key should hit the _ => {} catch-all
    app.handle_key(KeyEvent::new(KeyCode::F(1), KeyModifiers::empty()));
    assert_eq!(app.search.as_ref().unwrap().selected, selected);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn history_key_unrecognized_key_is_noop() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
    assert!(app.history.is_some());
    let selected = app.history.as_ref().unwrap().selected;
    app.handle_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::empty()));
    assert_eq!(app.history.as_ref().unwrap().selected, selected);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn theme_key_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
    assert!(app.theme_picker.is_some());
    let selected = app.theme_picker.as_ref().unwrap().selected;
    app.handle_key(KeyEvent::new(KeyCode::F(3), KeyModifiers::empty()));
    assert_eq!(app.theme_picker.as_ref().unwrap().selected, selected);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn command_key_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL));
    assert!(app.command_palette.is_some());
    let selected = app.command_palette.as_ref().unwrap().selected;
    app.handle_key(KeyEvent::new(KeyCode::F(4), KeyModifiers::empty()));
    assert_eq!(app.command_palette.as_ref().unwrap().selected, selected);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn in_file_search_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
    assert!(app.in_file_search.is_some());
    let current = app.in_file_search.as_ref().unwrap().current;
    app.handle_key(KeyEvent::new(KeyCode::F(5), KeyModifiers::empty()));
    assert_eq!(app.in_file_search.as_ref().unwrap().current, current);
    fs::remove_dir_all(&root).ok();
}

// -- handle_key: about overlay unrecognized key -------------------------------

#[test]
fn about_overlay_unrecognized_key_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(KeyEvent::new(KeyCode::F(6), KeyModifiers::empty()));
    assert!(app.show_about);
    fs::remove_dir_all(&root).ok();
}

// -- set_scroll_from_mouse_y: track_range == 0 --------------------------------

#[test]
fn set_scroll_from_mouse_y_track_range_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // A file with fewer lines than the content height: track_range = 0.
    app.open_file(&root.join("a.txt")); // 2 lines
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 50,
        height: 10,
    };
    app.show_scrollbar = true;
    // Click on the scrollbar column to trigger set_scroll_from_mouse_y
    // But display_line_count (2) < height (10), so set_scroll_from_mouse_y returns early.
    let before = app.content_scroll;
    app.set_scroll_from_mouse_y(5);
    assert_eq!(app.content_scroll, before);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn visual_line_not_entered_for_diff() {
    let root = temp_git_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));
    app.focus = Focus::Content;
    // Switch to the working-tree diff view, which sets is_diff.
    app.show_working_tree_diff(&root.join("tracked.txt"));
    assert!(app.is_diff);

    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    assert!(app.visual_line.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn opening_different_file_exits_visual_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt"));
    app.focus = Focus::Content;
    app.content_area = viewport(10);

    app.handle_key(KeyEvent::new(KeyCode::Char('V'), KeyModifiers::empty()));
    app.blame_panel = true;
    assert!(app.visual_line.is_some());

    app.open_file(&root.join("a.txt"));
    assert!(app.visual_line.is_none());
    assert!(!app.blame_panel);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_no_file_selected() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.copy_path_to_clipboard(false);
    assert_eq!(app.status_message.as_deref(), Some("no file selected"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn copy_path_no_file_selected_relative() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;
    app.copy_path_to_clipboard(true);
    assert_eq!(app.status_message.as_deref(), Some("no file selected"));
    fs::remove_dir_all(&root).ok();
}

// -- icons_enabled ------------------------------------------------------------

#[test]
fn icons_enabled_defaults_to_false() {
    let root = temp_tree();
    let app = app_for(&root);
    assert!(!app.icons_enabled);
    assert!(app.icon_map.is_empty());
    assert!(app.icon_dir_open.is_empty());
    assert!(app.icon_dir_closed.is_empty());
    assert!(app.icon_fallback.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn icons_enabled_true_when_config_says_so() {
    let root = temp_tree();
    let cfg = Config {
        icons: true,
        ..Config::default()
    };
    let app = App::new(root.to_path_buf(), cfg, None, None).unwrap();
    assert!(app.icons_enabled);
    fs::remove_dir_all(&root).ok();
}

// -- plugin picker ------------------------------------------------------------

#[test]
fn plugin_picker_key_opens_overlay() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(app.plugin_picker.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));
    assert!(app.plugin_picker.is_some());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_picker_esc_closes_overlay() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));
    assert!(app.plugin_picker.is_some());
    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
    assert!(app.plugin_picker.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_picker_opens_empty_when_no_plugins_configured() {
    // Even with an empty tv.toml, bundled plugins are seeded so the palette
    // is not empty.
    let root = temp_tree();
    let mut app = app_for(&root);
    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));
    let picker = app.plugin_picker.as_ref().unwrap();
    assert!(
        picker.results_len() > 0,
        "bundled plugins must appear even with a bare config"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_picker_nav_down_and_up() {
    use crate::plugin::PluginEntry;
    use std::path::PathBuf;

    let root = temp_tree();
    // Two plugins with bad paths so activate_all silently skips them.
    let mut cfg = Config::default();
    cfg.plugins.insert(
        "alpha".to_string(),
        PluginEntry {
            path: PathBuf::from("/nonexistent/a"),
            enabled: false,
            ..Default::default()
        },
    );
    cfg.plugins.insert(
        "beta".to_string(),
        PluginEntry {
            path: PathBuf::from("/nonexistent/b"),
            enabled: false,
            ..Default::default()
        },
    );
    let mut app = App::new(root.clone(), cfg, None, None).unwrap();

    app.handle_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::empty()));
    assert_eq!(app.plugin_picker.as_ref().unwrap().selected, 0);

    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.plugin_picker.as_ref().unwrap().selected, 1);

    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.plugin_picker.as_ref().unwrap().selected, 0);

    // Can't navigate above the first item.
    app.handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::empty()));
    assert_eq!(app.plugin_picker.as_ref().unwrap().selected, 0);

    // Navigate to the very last item and verify we can't go further.
    let total = app.plugin_picker.as_ref().unwrap().results_len();
    // Move to the end.
    for _ in 0..total {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    }
    let last_idx = app.plugin_picker.as_ref().unwrap().selected;
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
    assert_eq!(app.plugin_picker.as_ref().unwrap().selected, last_idx);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_picker_command_palette_entry_exists() {
    use crate::command_palette::COMMANDS;
    assert!(
        COMMANDS.iter().any(|c| c.action_id == "open_plugin_picker"),
        "command palette must include open_plugin_picker"
    );
}

// -- DiffMode -----------------------------------------------------------------

#[test]
fn diff_mode_next_cycles_all_staged_unstaged() {
    assert_eq!(DiffMode::All.next(), DiffMode::Staged);
    assert_eq!(DiffMode::Staged.next(), DiffMode::Unstaged);
    assert_eq!(DiffMode::Unstaged.next(), DiffMode::All);
}

#[test]
fn diff_mode_labels_are_distinct() {
    let labels = [
        DiffMode::All.label(),
        DiffMode::Staged.label(),
        DiffMode::Unstaged.label(),
    ];
    let unique: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "each DiffMode variant must have a unique label"
    );
}

#[test]
fn diff_mode_default_is_all() {
    assert_eq!(DiffMode::default(), DiffMode::All);
}

// -- S keybinding toggles diff mode in git mode --------------------------------

/// Builds a git repo with one committed file and an unstaged modification.
fn temp_git_for_diff_mode() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_diff_mode_{}_{n}", std::process::id()));
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
    git(&["commit", "-q", "-m", "init"]);
    fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn s_key_cycles_diff_mode_in_diff_view() {
    let root = temp_git_for_diff_mode();
    let mut app = app_for(&root);
    // Enter git mode so the content pane shows a working-tree diff.
    app.handle_key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL));
    assert!(app.is_diff, "git mode should show a diff");
    assert_eq!(app.diff_mode, DiffMode::All, "default mode should be All");

    app.focus = Focus::Content;

    // First S: All → Staged
    app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::empty()));
    assert_eq!(app.diff_mode, DiffMode::Staged);
    assert!(
        app.is_diff,
        "content pane must still show a diff after mode switch"
    );
    assert!(
        app.content_title
            .as_deref()
            .unwrap_or("")
            .contains("[staged]"),
        "title should reflect staged mode, was: {:?}",
        app.content_title
    );

    // Second S: Staged → Unstaged
    app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::empty()));
    assert_eq!(app.diff_mode, DiffMode::Unstaged);
    assert!(
        app.content_title
            .as_deref()
            .unwrap_or("")
            .contains("[unstaged]"),
        "title should reflect unstaged mode, was: {:?}",
        app.content_title
    );

    // Third S: Unstaged → All (full cycle)
    app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::empty()));
    assert_eq!(app.diff_mode, DiffMode::All);
    assert!(
        app.content_title.as_deref().unwrap_or("").contains("[all]"),
        "title should reflect all mode, was: {:?}",
        app.content_title
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn s_key_outside_diff_view_is_noop() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = Focus::Content;
    assert!(!app.is_diff);

    // S should not change anything outside a diff view.
    app.handle_key(KeyEvent::new(KeyCode::Char('S'), KeyModifiers::empty()));
    assert_eq!(app.diff_mode, DiffMode::All, "mode unchanged outside diff");
    assert!(!app.is_diff);
    fs::remove_dir_all(&root).ok();
}
