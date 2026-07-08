use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

fn temp_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_tree_{}_{}_{}", name, std::process::id(), n));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn dir_tree() -> PathBuf {
    let dir = temp_dir("dirs");
    fs::create_dir_all(dir.join("dir_a")).unwrap();
    fs::create_dir_all(dir.join("dir_b")).unwrap();
    fs::create_dir_all(dir.join(".hidden_dir")).unwrap();
    fs::write(dir.join("a.txt"), "").unwrap();
    fs::write(dir.join("b.txt"), "").unwrap();
    fs::write(dir.join("dir_a").join("c.txt"), "").unwrap();
    fs::write(dir.join(".hidden_file"), "").unwrap();
    fs::write(dir.join(".hidden_dir").join("f.txt"), "").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn dirs_before_files_same_extension_sorted_by_name() {
    let root = dir_tree();
    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["dir_a", "dir_b", "a.txt", "b.txt"]);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn files_sorted_by_extension_then_name() {
    let root = temp_dir("ext_sort");
    fs::create_dir_all(root.join("z_app")).unwrap();
    fs::write(root.join("main.rs"), "").unwrap();
    fs::write(root.join("lib.rs"), "").unwrap();
    fs::write(root.join("README.md"), "").unwrap();
    fs::write(root.join("Cargo.toml"), "").unwrap();
    fs::write(root.join("LICENSE"), "").unwrap();
    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    // dir first, then files grouped by extension: no-ext -> md -> rs -> toml
    assert_eq!(
        names,
        vec![
            "z_app",
            "LICENSE",
            "README.md",
            "lib.rs",
            "main.rs",
            "Cargo.toml"
        ]
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collapsed_dir_hides_children() {
    let root = dir_tree();
    let expanded = HashSet::new();
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    assert!(nodes.iter().all(|n| n.depth == 0));
    assert_eq!(nodes.len(), 4, "all root entries visible: {:?}", nodes);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn expanded_dir_shows_children_at_depth_1() {
    let root = dir_tree();
    let expanded = HashSet::from([root.clone(), root.join("dir_a")]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let c = nodes.iter().find(|n| n.name == "c.txt").unwrap();
    assert_eq!(c.depth, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn three_level_nesting_expands_correctly() {
    // Regression test for the single-walk rewrite of `build_visible`: entries
    // must only appear when every ancestor directory up to root is expanded,
    // and depth tracks the pre-collected `children` map correctly at 3+ levels.
    let root = temp_dir("deep");
    let a = root.join("a");
    let b = a.join("b");
    fs::create_dir_all(&b).unwrap();
    fs::write(b.join("deep.txt"), "").unwrap();

    // Neither "a" nor "a/b" expanded: only "a" itself is visible.
    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "a");

    // "a" expanded but not "a/b": "b" is visible, "deep.txt" is not.
    let expanded = HashSet::from([root.clone(), a.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);

    // Both "a" and "a/b" expanded: "deep.txt" appears at depth 2.
    let expanded = HashSet::from([root.clone(), a.clone(), b.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let deep = nodes.iter().find(|n| n.name == "deep.txt").unwrap();
    assert_eq!(deep.depth, 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collapsed_sibling_dir_children_are_not_walked_deeper_than_one_level() {
    // A collapsed top-level directory must not have anything beneath its
    // first level leak into the output.
    let root = temp_dir("collapsed_sibling");
    let collapsed = root.join("collapsed");
    fs::create_dir_all(collapsed.join("nested")).unwrap();
    fs::write(collapsed.join("nested").join("f.txt"), "").unwrap();
    fs::write(root.join("visible.txt"), "").unwrap();

    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["collapsed", "visible.txt"]);
    assert!(!names.contains(&"nested"));
    assert!(!names.contains(&"f.txt"));
    fs::remove_dir_all(&root).ok();
}

#[cfg(unix)]
#[test]
fn collapsed_top_level_dir_contents_are_never_read() {
    // Regression test: the walker must not descend into a collapsed
    // top-level directory at all, not even one level, to build the tree. A
    // permission-denied grandchild inside a collapsed dir must therefore
    // never surface as a walk error, since the walker never opens it.
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("collapsed_perm");
    let collapsed = root.join("collapsed");
    let locked = collapsed.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(root.join("visible.txt"), "").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let expanded = HashSet::from([root.clone()]);
    let (nodes, errors) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["collapsed", "visible.txt"]);
    assert_eq!(
        errors, 0,
        "a collapsed directory's unreadable children must not surface as walk errors"
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn flat_tree_with_nothing_expanded_skips_deep_walk_but_output_is_unchanged() {
    // Regression test for the build_visible double-walk fix: when no root
    // child directory is expanded, the deep walk is skipped entirely (it can
    // never yield a depth >= 2 entry), but the visible output must be
    // identical to what the (now-skipped) deep walk would have produced.
    let root = temp_dir("flat_guard");
    fs::create_dir_all(root.join("dir_a")).unwrap();
    fs::create_dir_all(root.join("dir_b")).unwrap();
    fs::write(root.join("dir_a").join("nested.txt"), "").unwrap();
    fs::write(root.join("dir_b").join("nested.txt"), "").unwrap();
    fs::write(root.join("a.txt"), "").unwrap();

    let (nodes, errors) = build_visible(&root, &HashSet::new(), false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["dir_a", "dir_b", "a.txt"]);
    assert!(nodes.iter().all(|n| n.depth == 0));
    assert_eq!(errors, 0);
    fs::remove_dir_all(&root).ok();
}

#[cfg(unix)]
#[test]
fn flat_tree_with_nothing_expanded_does_not_surface_errors_from_unreadable_dirs() {
    // With `expanded` completely empty (not even root), an unreadable
    // grandchild inside a collapsed directory must never be opened or
    // counted as a walk error. Note: this holds whether the deep walk is
    // skipped by the guard above or merely blocked by `filter_entry` at
    // depth 1, so this test does not by itself prove the guard fires - it
    // only asserts the (guard-independent) invariant that collapsed
    // directories' contents are never read.
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir("flat_guard_perm");
    let collapsed = root.join("collapsed");
    let locked = collapsed.join("locked");
    fs::create_dir_all(&locked).unwrap();
    fs::write(root.join("visible.txt"), "").unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

    let (nodes, errors) = build_visible(&root, &HashSet::new(), false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, vec!["collapsed", "visible.txt"]);
    assert_eq!(
        errors, 0,
        "no walk errors should surface for an unreadable grandchild when nothing is expanded"
    );

    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    fs::remove_dir_all(&root).ok();
}

#[test]
fn one_expanded_sibling_among_collapsed_dirs_still_triggers_deep_walk() {
    // Guard must only skip the deep walk when NO root child is expanded; as
    // soon as one is, the deep walk still runs and reveals its contents while
    // leaving collapsed siblings untouched.
    let root = temp_dir("mixed_guard");
    let expanded_dir = root.join("expanded_dir");
    let collapsed_dir = root.join("collapsed_dir");
    fs::create_dir_all(expanded_dir.join("nested")).unwrap();
    fs::write(expanded_dir.join("nested").join("inner.txt"), "").unwrap();
    fs::create_dir_all(collapsed_dir.join("nested")).unwrap();
    fs::write(collapsed_dir.join("nested").join("inner.txt"), "").unwrap();

    let expanded = HashSet::from([expanded_dir.clone(), expanded_dir.join("nested")]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());

    let inner_matches: Vec<&TreeNode> = nodes.iter().filter(|n| n.name == "inner.txt").collect();
    assert_eq!(
        inner_matches.len(),
        1,
        "only the expanded sibling's inner.txt should surface: {:?}",
        nodes
    );
    assert_eq!(inner_matches[0].depth, 2);
    assert!(nodes
        .iter()
        .any(|n| n.name == "collapsed_dir" && n.depth == 0));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn show_hidden_reveals_dotfiles() {
    let root = dir_tree();
    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, true, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&".hidden_file"));
    assert!(names.contains(&".hidden_dir"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn hide_hidden_omits_dotfiles() {
    let root = dir_tree();
    let expanded = HashSet::from([root.clone()]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(!names.contains(&".hidden_file"));
    assert!(!names.contains(&".hidden_dir"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ghost_nodes_appear_for_deleted_files() {
    let root = dir_tree();
    let expanded = HashSet::from([root.clone(), root.join("dir_a")]);
    let deleted = HashSet::from([root.join("gone.txt"), root.join("dir_a").join("missing.rs")]);
    let (nodes, _) = build_visible(&root, &expanded, false, true, &deleted);

    let gone = nodes.iter().find(|n| n.name == "gone.txt").unwrap();
    assert!(gone.deleted);
    assert_eq!(gone.depth, 0);

    let missing = nodes.iter().find(|n| n.name == "missing.rs").unwrap();
    assert!(missing.deleted);
    assert_eq!(missing.depth, 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collect_all_files_returns_flat_file_list() {
    let root = dir_tree();
    let files = collect_all_files(&root, false, true);
    let names: Vec<&str> = files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_str().unwrap())
        .collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"c.txt"));
    assert!(!names.contains(&"dir_a"));
    assert!(!names.contains(&".hidden_file"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn collect_all_files_with_hidden_includes_dotfiles() {
    let root = dir_tree();
    let files = collect_all_files(&root, true, true);
    let names: Vec<&str> = files
        .iter()
        .filter_map(|p| p.file_name())
        .map(|n| n.to_str().unwrap())
        .collect();
    assert!(names.contains(&".hidden_file"));
    assert!(names.contains(&"f.txt"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn empty_directory_yields_no_nodes() {
    let root = temp_dir("empty");
    let (nodes, _) = build_visible(&root, &HashSet::new(), false, true, &HashSet::new());
    assert!(nodes.is_empty());
    let files = collect_all_files(&root, false, true);
    assert!(files.is_empty());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn ignore_gitignore_respects_rules() {
    use std::process::Command;
    let root = temp_dir("ggi");
    let git = |args: &[&str]| {
        Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
            .args(args)
            .status()
            .unwrap();
    };
    git(&["init", "-q"]);
    fs::write(root.join("tracked.txt"), "").unwrap();
    fs::write(root.join("ignored.log"), "").unwrap();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    let expanded = HashSet::from([root.clone()]);

    let (nodes, _) = build_visible(&root, &expanded, false, false, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(!names.contains(&"ignored.log"));

    let (nodes, _) = build_visible(&root, &expanded, false, true, &HashSet::new());
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"ignored.log"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn build_visible_runs_without_panicking() {
    let root = temp_dir("dummy_test");
    let expanded = HashSet::new();
    let (nodes, _) = build_visible(&root, &expanded, false, false, &HashSet::new());
    assert!(nodes.is_empty());
    fs::remove_dir_all(&root).ok();
}
