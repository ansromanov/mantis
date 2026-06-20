use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn temp_tree() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("tv_nav_test_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("b.txt"), "hello\n").unwrap();
    fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
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
