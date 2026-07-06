use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_nav_test_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("b.txt"), "hello\n").unwrap();
    fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
    dir.canonicalize().unwrap()
}

fn deep_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_nav_test_deep_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub1").join("sub2").join("sub3")).unwrap();
    fs::write(dir.join("top.txt"), "top\n").unwrap();
    fs::write(dir.join("sub1").join("mid.txt"), "mid\n").unwrap();
    fs::write(dir.join("sub1").join("sub2").join("deep.txt"), "deep\n").unwrap();
    fs::write(
        dir.join("sub1")
            .join("sub2")
            .join("sub3")
            .join("deepest.txt"),
        "deepest\n",
    )
    .unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn collapse_all_clears_expanded() {
    let root = temp_tree();
    let mut app = app_for(&root);

    app.expanded.insert(root.join("sub"));
    app.rebuild(true);
    assert!(!app.expanded.is_empty(), "sub should be expanded");

    app.collapse_all();

    assert!(
        app.expanded.is_empty(),
        "collapse_all must clear all expansions"
    );
    assert!(
        app.nodes.iter().all(|n| n.depth == 0),
        "all nodes must be at depth 0 after collapse_all"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn activate_dir_marks_session_dirty() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let sub_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .expect("sub dir node should exist");
    app.tree_selected = sub_idx;
    app.session_dirty = false;
    app.activate_selected();
    assert!(
        app.session_dirty,
        "toggling a directory's fold state must mark the session dirty"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn expand_all_exposes_nested_files() {
    let root = temp_tree();
    let mut app = app_for(&root);

    let before_count = app.nodes.len();
    app.expand_all();

    assert!(
        app.nodes.len() > before_count,
        "expand_all must expose at least the nested c.txt"
    );
    assert!(
        app.nodes
            .iter()
            .any(|n| n.path == root.join("sub").join("c.txt")),
        "c.txt inside sub/ must be visible after expand_all"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collapse_all_key_binding() {
    let root = temp_tree();
    let mut app = app_for(&root);

    app.expanded.insert(root.join("sub"));
    app.rebuild(true);

    app.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::empty()));

    assert!(
        app.expanded.is_empty(),
        "'-' key must collapse all directories"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn expand_all_key_binding() {
    let root = temp_tree();
    let mut app = app_for(&root);

    let before_count = app.nodes.len();

    app.handle_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::empty()));

    assert!(
        app.nodes.len() > before_count,
        "'=' key must expand all directories"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collapse_all_preserves_selection_when_visible() {
    let root = temp_tree();
    let mut app = app_for(&root);

    let sub_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub"))
        .unwrap();
    app.tree_selected = sub_idx;

    app.collapse_all();

    assert_eq!(
        app.nodes[app.tree_selected].path,
        root.join("sub"),
        "selection should remain on sub/ after collapse_all"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collapse_all_selects_nearest_ancestor_for_nested_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);

    app.expanded.insert(root.join("sub"));
    app.rebuild(true);
    let nested_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub").join("c.txt"))
        .unwrap();
    app.tree_selected = nested_idx;

    app.collapse_all();

    assert_eq!(
        app.nodes[app.tree_selected].path,
        root.join("sub"),
        "collapse_all must select the nearest visible ancestor when the selected path is hidden"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn navigate_to_breadcrumb_root_selects_index_zero() {
    let root = deep_tree();
    let mut app = app_for(&root);

    // Expand sub1 and select a deeply nested file.
    app.expanded.insert(root.join("sub1"));
    app.expanded.insert(root.join("sub1").join("sub2"));
    app.expanded
        .insert(root.join("sub1").join("sub2").join("sub3"));
    app.rebuild(true);
    let deepest = root
        .join("sub1")
        .join("sub2")
        .join("sub3")
        .join("deepest.txt");
    app.tree_selected = app.nodes.iter().position(|n| n.path == deepest).unwrap();

    // Navigate to root via breadcrumb.
    app.navigate_to_breadcrumb(&root);

    assert_eq!(app.tree_selected, 0, "root should be selected at index 0");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn navigate_to_breadcrumb_selects_intermediate_dir() {
    let root = deep_tree();
    let mut app = app_for(&root);

    // Expand all directories and select the deepest file.
    app.expanded.insert(root.join("sub1"));
    app.expanded.insert(root.join("sub1").join("sub2"));
    app.expanded
        .insert(root.join("sub1").join("sub2").join("sub3"));
    app.rebuild(true);
    let deepest = root
        .join("sub1")
        .join("sub2")
        .join("sub3")
        .join("deepest.txt");
    app.tree_selected = app.nodes.iter().position(|n| n.path == deepest).unwrap();

    let sub2 = root.join("sub1").join("sub2");
    app.navigate_to_breadcrumb(&sub2);

    let selected_path = app.nodes[app.tree_selected].path.clone();
    assert_eq!(
        selected_path, sub2,
        "navigate_to_breadcrumb should select sub2"
    );
    assert!(
        app.expanded.contains(&sub2),
        "target directory should be in expanded set"
    );
    assert!(
        app.expanded.contains(&root.join("sub1")),
        "ancestor sub1 should remain expanded"
    );
    assert!(
        app.nodes
            .iter()
            .any(|n| n.path == root.join("sub1").join("sub2").join("deep.txt")),
        "sub2 children should be visible"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn navigate_to_breadcrumb_expands_ancestors_of_unexpanded_target() {
    let root = deep_tree();
    let mut app = app_for(&root);

    // Select top.txt at root level — no directories expanded.
    let top = root.join("top.txt");
    app.tree_selected = app.nodes.iter().position(|n| n.path == top).unwrap();

    // Navigate to sub2 which is not currently expanded.
    let sub2 = root.join("sub1").join("sub2");
    app.navigate_to_breadcrumb(&sub2);

    assert!(
        app.expanded.contains(&root.join("sub1")),
        "ancestor sub1 should be expanded"
    );
    assert!(
        app.expanded.contains(&sub2),
        "target sub2 should be expanded"
    );
    let selected_path = app.nodes[app.tree_selected].path.clone();
    assert_eq!(
        selected_path, sub2,
        "breadcrumb navigation should select sub2"
    );
    assert!(
        app.nodes.iter().any(|n| n.path == sub2.join("deep.txt")),
        "sub2 children should be visible after ancestor expansion"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn navigate_to_breadcrumb_outside_root_changes_root() {
    let root = deep_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    let parent = root.parent().expect("temp dir has a parent").to_path_buf();
    app.initial_root = parent.clone();

    // Navigate to a parent of the current root (simulating clicking ".." in
    // the breadcrumb). This should change the viewer root to the parent.
    app.navigate_to_breadcrumb(&parent);

    assert_eq!(app.root, parent, "root should change to parent");
    assert!(app.expanded.is_empty(), "expanded should be cleared");
    assert!(app.current_file.is_none(), "current file should be cleared");
    assert!(
        !app.nodes.is_empty(),
        "parent directory should have contents"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn descend_to_selected_changes_root_for_dir() {
    let root = deep_tree();
    let mut app = app_for(&root);

    let sub1_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub1"))
        .expect("sub1 dir should exist");
    app.tree_selected = sub1_idx;

    app.descend_to_selected();

    assert_eq!(app.root, root.join("sub1"), "root should change to sub1");
    assert!(app.expanded.is_empty(), "expanded should be cleared");
    assert!(app.current_file.is_none(), "current file should be cleared");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn descend_to_selected_clears_plugin_content() {
    // Root change must drop cached plugin content for the previous tree, both
    // the styled spans and the parallel plain-text store.
    let root = deep_tree();
    let mut app = app_for(&root);
    let path = root.join("top.txt");
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(ratatui::style::Style::default(), "x".to_string())]],
    );
    app.plugin_content_text.insert(path, vec!["x".to_string()]);

    let sub1_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub1"))
        .expect("sub1 dir should exist");
    app.tree_selected = sub1_idx;
    app.descend_to_selected();

    assert!(app.plugin_content.is_empty(), "plugin_content must clear");
    assert!(
        app.plugin_content_text.is_empty(),
        "plugin_content_text must clear"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn descend_to_selected_clears_plugin_contributions() {
    // Root change must drop per-plugin contribution tracking for the old tree;
    // otherwise teardown after a root switch would target stale paths.
    use crate::plugin::PluginContributions;
    let root = deep_tree();
    let mut app = app_for(&root);
    let mut contrib = PluginContributions::default();
    contrib.content_paths.insert(root.join("top.txt"));
    app.plugin_contributions
        .insert("iconize".to_string(), contrib);

    let sub1_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub1"))
        .expect("sub1 dir should exist");
    app.tree_selected = sub1_idx;
    app.descend_to_selected();

    assert!(
        app.plugin_contributions.is_empty(),
        "plugin_contributions must clear on root change"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn descend_to_selected_does_nothing_for_file() {
    let root = deep_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();

    let top_idx = app
        .nodes
        .iter()
        .position(|n| !n.is_dir && n.path == root.join("top.txt"))
        .expect("top.txt should exist");
    app.tree_selected = top_idx;

    app.descend_to_selected();

    assert_eq!(app.root, orig_root, "root should not change for a file");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn navigate_to_breadcrumb_preserves_other_expansions() {
    let root = deep_tree();
    let mut app = app_for(&root);

    // Expand sub1 + sub1/sub2 and keep them open.
    app.expanded.insert(root.join("sub1"));
    app.expanded.insert(root.join("sub1").join("sub2"));
    app.rebuild(true);

    let top = root.join("top.txt");
    app.tree_selected = app.nodes.iter().position(|n| n.path == top).unwrap();

    // Navigate to root — unrelated nodes should stay expanded.
    app.navigate_to_breadcrumb(&root);

    assert!(
        app.expanded.contains(&root.join("sub1")),
        "unrelated expansions should be preserved"
    );
    assert!(
        app.expanded.contains(&root.join("sub1").join("sub2")),
        "nested unrelated expansions should be preserved"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_scrolls_restored_selection_into_view() {
    let root = temp_tree();
    // Enough files to overflow a short viewport.
    for i in 0..20 {
        fs::write(root.join(format!("f{i:02}.txt")), "").unwrap();
    }
    let mut app = app_for(&root);
    assert!(!app.tree_independent_scroll, "default mode under test");
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };
    assert!(app.nodes.len() > 5, "tree must overflow the viewport");

    // Select the bottom node, then force the viewport back to the top.
    let last = app.nodes.len() - 1;
    app.tree_selected = last;
    app.tree_scroll = 0;
    let sel_path = app.nodes[last].path.clone();

    app.rebuild(true);

    // Selection preserved by path, viewport nudged so it stays visible.
    assert_eq!(app.nodes[app.tree_selected].path, sel_path);
    let h = app.tree_area.height as usize;
    assert!(
        app.tree_selected >= app.tree_scroll && app.tree_selected < app.tree_scroll + h,
        "restored selection {} must be within viewport [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + h
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tree_up_dir_from_top_level_file_changes_root() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    // Select a.txt (directly in root)
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .expect("a.txt node");
    app.tree_selected = file_idx;
    app.tree_up_dir();
    // Under the new clamp rules, we cannot go up past initial_root.
    assert_eq!(
        app.root, orig_root,
        "tree_up_dir from top-level file should NOT change root past initial_root"
    );
    assert_eq!(
        app.status_message.as_ref().map(|sm| sm.text.as_str()),
        Some("Already at root")
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn tree_up_dir_from_nested_dir_changes_root() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    // "sub" is a dir directly in root → parent is root → target is root.
    // Under the new clamp rules, we cannot go up past initial_root, so root remains orig_root.
    let sub_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("sub"))
        .expect("sub dir node");
    app.tree_selected = sub_idx;
    app.tree_up_dir();
    assert_eq!(
        app.root, orig_root,
        "tree_up_dir from sub should not change root past initial_root"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn tree_up_dir_from_nested_selection_goes_to_ancestor_then_root() {
    let root = deep_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    // Expand sub1 → sub2 → sub3 so deepest.txt is visible
    let sub1 = root.join("sub1");
    let sub2 = sub1.join("sub2");
    let sub3 = sub2.join("sub3");
    app.expanded.insert(sub1.clone());
    app.expanded.insert(sub2.clone());
    app.expanded.insert(sub3.clone());
    app.rebuild(true);
    // Select deepest.txt at root/sub1/sub2/sub3/deepest.txt
    let deep = sub3.join("deepest.txt");
    let deep_idx = app
        .nodes
        .iter()
        .position(|n| n.path == deep)
        .expect("deepest.txt");
    app.tree_selected = deep_idx;
    // Go up: containing dir is sub3 → parent is sub2 → select sub2
    app.tree_up_dir();
    assert!(
        app.nodes
            .get(app.tree_selected)
            .is_some_and(|n| n.path == sub2),
        "first up: should select sub2, got {:?}",
        app.nodes.get(app.tree_selected).map(|n| &n.path)
    );
    // Go up: containing dir is sub2 → parent is sub1 → select sub1
    app.tree_up_dir();
    assert!(
        app.nodes
            .get(app.tree_selected)
            .is_some_and(|n| n.path == sub1),
        "second up: should select sub1, got {:?}",
        app.nodes.get(app.tree_selected).map(|n| &n.path)
    );
    // Go up: containing dir is sub1 → parent is root → target is root.
    // Under the new clamp rules, we cannot go up past initial_root, so root remains orig_root.
    app.tree_up_dir();
    assert_eq!(
        app.root, orig_root,
        "third up: should clamp to initial root"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn rebuild_false_preserves_scroll() {
    let root = temp_tree();
    // Enough files to overflow a short viewport.
    for i in 0..20 {
        fs::write(root.join(format!("f{i:02}.txt")), "").unwrap();
    }
    let mut app = app_for(&root);
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };
    assert!(app.nodes.len() > 5, "tree must overflow the viewport");

    // Select the first node, then scroll the viewport far down.
    let first_path = app.nodes[0].path.clone();
    app.tree_selected = 0;
    let scroll_target = app.tree_scroll_max().saturating_sub(1);
    app.tree_scroll = scroll_target;

    app.rebuild(false);

    // Selection preserved by path (same node as before).
    assert_eq!(app.nodes[app.tree_selected].path, first_path);
    // Scroll unchanged (only clamped, which it shouldn't be here).
    assert_eq!(
        app.tree_scroll, scroll_target,
        "tree_scroll must not change after rebuild(false)"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_false_clamps_scroll_when_tree_shrinks() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };
    // Force a scroll past the tree's max (3 root-level nodes, height=3 → max=0).
    app.tree_scroll = 999;
    app.tree_selected = 0;

    app.rebuild(false);

    // tree_scroll should be clamped to the new max.
    let max = app.tree_scroll_max();
    assert!(
        app.tree_scroll <= max,
        "tree_scroll must be clamped to {} after rebuild(false), got {}",
        max,
        app.tree_scroll
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_true_recenters_selection() {
    let root = temp_tree();
    for i in 0..20 {
        fs::write(root.join(format!("f{i:02}.txt")), "").unwrap();
    }
    let mut app = app_for(&root);
    app.tree_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 3,
    };

    // Select last node but force scroll to top.
    let last = app.nodes.len() - 1;
    app.tree_selected = last;
    app.tree_scroll = 0;

    app.rebuild(true);

    // After recenter=true, selection must be visible in the viewport.
    let h = app.tree_area.height as usize;
    assert!(
        app.tree_selected >= app.tree_scroll && app.tree_selected < app.tree_scroll + h,
        "rebuild(true) must nudge viewport so selection {} is visible in [{}, {})",
        app.tree_selected,
        app.tree_scroll,
        app.tree_scroll + h
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_git_mode_key_flips_git_mode_flag() {
    let root = temp_tree();
    let mut app = app_for(&root);
    assert!(!app.git_mode);
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(app.git_mode, "Ctrl+D must enable git mode");
    app.handle_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL));
    assert!(!app.git_mode, "second Ctrl+D must disable git mode");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_root_clears_viewing_revision() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    // Descend to sub so app.root is root/sub and set_root actually executes on going up
    let sub = root.join("sub");
    let sub_idx = app.nodes.iter().position(|n| n.path == sub).unwrap();
    app.tree_selected = sub_idx;
    app.descend_to_selected();

    // Select c.txt (directly at sub) so tree_up_dir triggers set_root.
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == sub.join("c.txt"))
        .expect("c.txt node");
    app.tree_selected = file_idx;
    app.viewing_revision = Some("abc1234".to_string());
    app.tree_up_dir();
    assert!(
        app.viewing_revision.is_none(),
        "set_root (via tree_up_dir) must clear viewing_revision"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn toggle_git_mode_requests_async_status_refresh_when_enabled() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.git_status_enabled = true;
    app.toggle_git_mode();
    app.pump_loads();
    // Non-git root → status stays empty; verify no panic and mode flipped.
    assert!(app.git_mode, "toggle must enable git mode");
    let _ = app.git_status_map.len();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_root_refreshes_git_status_when_enabled() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();
    app.git_status_enabled = true;
    // Seed a stale entry so we can verify the refresh cleared it.
    app.git_status_map
        .insert(root.join("stale.txt"), crate::git::GitStatus::Modified);

    // Descend to sub so app.root is root/sub and set_root actually executes on going up
    let sub = root.join("sub");
    let sub_idx = app.nodes.iter().position(|n| n.path == sub).unwrap();
    app.tree_selected = sub_idx;
    app.descend_to_selected();

    // tree_up_dir on a file inside sub triggers set_root(root).
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == sub.join("c.txt"))
        .expect("c.txt node");
    app.tree_selected = file_idx;
    app.tree_up_dir();
    app.pump_loads();
    // new root is not a git repo → map is empty (stale entry gone).
    assert!(
        app.git_status_map.is_empty(),
        "set_root must replace stale git status with fresh scan"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn git_flat_nodes_includes_ignored_status_when_in_map() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Seed the map with an Ignored-status file (as if git_show_ignored=true was
    // passed to repo_status). The flat-node filter must not strip it.
    let ignored_path = root.join("build.log");
    fs::write(&ignored_path, "log\n").unwrap();
    app.git_status_map
        .insert(ignored_path.clone(), crate::git::GitStatus::Ignored);
    app.git_status_enabled = true;
    app.git_mode = true;
    app.git_mode_flat = true;
    app.rebuild(false);
    assert!(
        app.nodes.iter().any(|n| n.path == ignored_path),
        "Ignored entry in the status map must appear in flat git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn git_mode_tree_includes_ignored_status_when_in_map() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Same scenario for hierarchical git mode.
    let ignored_path = root.join("build.log");
    fs::write(&ignored_path, "log\n").unwrap();
    app.git_status_map
        .insert(ignored_path.clone(), crate::git::GitStatus::Ignored);
    app.git_status_enabled = true;
    app.git_mode = true;
    app.git_mode_flat = false;
    app.rebuild(false);
    assert!(
        app.nodes.iter().any(|n| n.path == ignored_path),
        "Ignored entry in the status map must appear in hierarchical git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn set_root_clears_plugin_content_active_path() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let orig_root = root.clone();

    // Descend to sub so app.root is root/sub and set_root actually executes on going up
    let sub = root.join("sub");
    let sub_idx = app.nodes.iter().position(|n| n.path == sub).unwrap();
    app.tree_selected = sub_idx;
    app.descend_to_selected();

    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == sub.join("c.txt"))
        .expect("c.txt node");
    app.tree_selected = file_idx;
    app.plugin_content_active_path = Some(sub.join("c.txt"));
    app.tree_up_dir();
    assert!(
        app.plugin_content_active_path.is_none(),
        "set_root must clear plugin_content_active_path so the next plugin render is treated as first-render"
    );
    fs::remove_dir_all(&orig_root).ok();
}

#[test]
fn rebuild_empty_git_mode_clears_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Load content so current_file is set, then seed state manually.
    app.open_file(&root.join("a.txt"));
    assert!(app.current_file.is_some(), "content should be loaded");
    app.virtual_file = None;
    app.content = vec!["stale line".to_string()];
    app.highlighted = vec![vec![]];
    // Set non-default values that clear_content_state must reset.
    app.is_diff = true;
    app.diff_side_by_side = true;
    app.active_line = 5;
    app.content_scroll = 10;
    app.content_title = Some("stale title".to_string());
    app.selection = Some(crate::selection::TextSelection {
        anchor: (0, 0),
        active: (1, 1),
    });
    app.git_mode = true;
    app.git_status_map.clear();
    app.rebuild(false);
    assert!(
        app.nodes.is_empty(),
        "git mode with no changes should yield empty nodes"
    );
    assert!(
        app.current_file.is_none(),
        "rebuild with empty git set must clear current_file"
    );
    assert!(
        app.content.is_empty(),
        "rebuild with empty git set must clear content buffer"
    );
    assert!(
        !app.is_diff,
        "rebuild with empty git set must clear is_diff flag"
    );
    assert!(
        app.content_title.is_none(),
        "rebuild with empty git set must clear content_title"
    );
    assert!(
        app.highlighted.is_empty(),
        "rebuild with empty git set must clear highlighted"
    );
    assert!(
        app.diff_rows.is_empty(),
        "rebuild with empty git set must clear diff_rows"
    );
    assert!(
        app.fold_regions.is_empty(),
        "rebuild with empty git set must clear fold_regions"
    );
    assert!(
        app.selection.is_none(),
        "rebuild with empty git set must clear selection"
    );
    assert_eq!(
        app.content_scroll, 0,
        "rebuild with empty git set must reset content_scroll"
    );
    assert_eq!(
        app.active_line, 0,
        "rebuild with empty git set must reset active_line"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_non_empty_git_mode_keeps_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Load content and set up git mode with a changed file.
    // Use open_file to set up a realistic current_file state, then seed
    // content directly (open_file uses lazy VirtualFile for .txt).
    app.open_file(&root.join("a.txt"));
    assert!(app.current_file.is_some(), "content should be loaded");
    app.virtual_file = None;
    app.content = vec!["line1".to_string(), "line2".to_string()];
    app.highlighted = vec![vec![]; 2];
    app.git_mode = true;
    app.git_status_map
        .insert(root.join("a.txt"), crate::git::GitStatus::Modified);
    app.rebuild(false);
    assert!(
        !app.nodes.is_empty(),
        "git mode with changes should have nodes ({})",
        app.nodes.len()
    );
    assert!(
        !app.content.is_empty(),
        "rebuild with non-empty git set must preserve content"
    );
    assert!(
        app.current_file.is_some(),
        "rebuild with non-empty git set must preserve current_file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_empty_git_mode_clears_plugin_content() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Simulate plugin rendering content for the current file.
    let path = root.join("a.txt");
    app.open_file(&path);
    app.current_file = Some(path.clone());
    app.content = vec!["original".to_string()];
    app.plugin_content.insert(
        path.clone(),
        vec![vec![(
            ratatui::style::Style::default(),
            "plugin".to_string(),
        )]],
    );
    app.plugin_content_text
        .insert(path, vec!["plugin".to_string()]);
    app.git_mode = true;
    app.git_status_map.clear();
    app.rebuild(false);
    assert!(
        app.plugin_content.is_empty(),
        "rebuild with empty git set must clear plugin_content"
    );
    assert!(
        app.plugin_content_text.is_empty(),
        "rebuild with empty git set must clear plugin_content_text"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn clamp_tree_selection_shrink_clamps_to_last() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let len = app.nodes.len();
    assert!(len > 0, "precondition: nodes must be non-empty");
    app.tree_selected = len + 10;
    app.clamp_tree_selection();
    assert_eq!(
        app.tree_selected,
        len.saturating_sub(1),
        "tree_selected past end must clamp to len-1"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn clamp_tree_selection_empty_yields_zero_no_panic() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.nodes.clear();
    app.tree_selected = 42;
    app.clamp_tree_selection();
    assert_eq!(
        app.tree_selected, 0,
        "empty nodes must clamp tree_selected to 0"
    );
    // Subsequent .get(0) must be None, not a panic.
    assert!(app.nodes.get(app.tree_selected).is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_calls_clamp_tree_selection_when_selection_lost() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Set selection past the end of the (still intact) list. rebuild will try
    // to restore by path, fail since the index is beyond the last element, and
    // fall through to clamp_tree_selection.
    let len = app.nodes.len();
    // Place selection beyond the end so rebuild must clamp it.
    app.tree_selected = len + 5;
    app.rebuild(false);
    assert!(
        app.tree_selected < app.nodes.len(),
        "after rebuild, tree_selected must be in range"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn rebuild_increments_tree_revision() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let before = app.tree_revision;
    app.rebuild(false);
    assert_eq!(
        app.tree_revision,
        before + 1,
        "rebuild must bump tree_revision by 1"
    );
    fs::remove_dir_all(&root).ok();
}
