use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
    let dir = std::env::temp_dir().join(format!("tv_nav_test_{}_{n}", std::process::id()));
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
    app.rebuild();
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
    app.rebuild();

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
    app.rebuild();
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
    app.rebuild();
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
    app.rebuild();
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

    // Navigate to a parent of the current root (simulating clicking ".." in
    // the breadcrumb). This should change the viewer root to the parent.
    let parent = root.parent().expect("temp dir has a parent").to_path_buf();
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
    app.rebuild();

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
