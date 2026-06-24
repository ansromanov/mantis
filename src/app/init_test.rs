//! Tests for `App::new` construction (see `init.rs`).
//!
//! These cover the directory-walk and config-driven visibility behaviour the
//! constructor is responsible for. Git-status seeding is exercised separately
//! in the git-mode tests in `mod_test.rs`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::config::Config;

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_init_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn new_app(root: &std::path::Path, cfg: Config) -> App {
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn app_new_builds_visible_root_tree() {
    let root = temp_dir();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "two\n").unwrap();

    let app = new_app(&root, Config::default());

    assert_eq!(app.tree_selected, 0);
    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"a.txt"), "got {names:?}");
    assert!(names.contains(&"b.txt"), "got {names:?}");
    assert!(names.contains(&"sub"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_hides_dotfiles_by_default() {
    let root = temp_dir();
    fs::write(root.join("visible.txt"), "x\n").unwrap();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let app = new_app(&root, Config::default());

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"visible.txt"), "got {names:?}");
    assert!(
        !names.contains(&".hidden"),
        "dotfile must be hidden; got {names:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_show_hidden_includes_dotfiles() {
    let root = temp_dir();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let cfg = Config {
        show_hidden: true,
        ..Config::default()
    };
    let app = new_app(&root, cfg);

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&".hidden"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_preserves_root_path() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.root, root);
    fs::remove_dir_all(&root).ok();
}
