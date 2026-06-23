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
