use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// A single entry in the flat file tree. `depth` controls indentation;
/// `deleted` marks a file that no longer exists on disk but is tracked by git.
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    /// Ghost node for a file deleted from the working tree but tracked by git.
    pub deleted: bool,
}

/// Recursively walks `root` using `ignore::WalkBuilder`, returning a flat
/// `Vec<TreeNode>` of files and directories. Only directories in `expanded`
/// are descended into. `deleted_files` are appended as ghost nodes.
pub fn build_visible(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    ignore_gitignore: bool,
    deleted_files: &HashSet<PathBuf>,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    collect(
        root,
        0,
        expanded,
        show_hidden,
        ignore_gitignore,
        deleted_files,
        &mut nodes,
    );
    nodes
}

/// Recursive helper for `build_visible`. Lists a single directory's entries
/// (depth 1 via `WalkBuilder`), sorts dirs before files, and recurses into
/// expanded directories.
fn collect(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    ignore_gitignore: bool,
    deleted_files: &HashSet<PathBuf>,
    out: &mut Vec<TreeNode>,
) {
    let mut entries: Vec<_> = WalkBuilder::new(dir)
        .max_depth(Some(1))
        .hidden(!show_hidden)
        .git_ignore(!ignore_gitignore)
        .git_global(!ignore_gitignore)
        .git_exclude(!ignore_gitignore)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() == 1)
        .collect();

    entries.sort_by(|a, b| {
        let ad = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let bd = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        bd.cmp(&ad).then_with(|| a.file_name().cmp(b.file_name()))
    });

    for e in entries {
        let path = e.path().to_path_buf();
        let name = match path.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);

        out.push(TreeNode {
            path: path.clone(),
            name,
            depth,
            is_dir,
            deleted: false,
        });

        if is_dir && expanded.contains(&path) {
            collect(
                &path,
                depth + 1,
                expanded,
                show_hidden,
                ignore_gitignore,
                deleted_files,
                out,
            );
        }
    }

    // Append ghost nodes for files deleted from the working tree. They go after
    // all real entries in this directory, sorted by name.
    let mut ghosts: Vec<&PathBuf> = deleted_files
        .iter()
        .filter(|p| p.parent() == Some(dir))
        .collect();
    ghosts.sort();
    for p in ghosts {
        let name = match p.file_name() {
            Some(n) => n.to_string_lossy().to_string(),
            None => continue,
        };
        out.push(TreeNode {
            path: p.clone(),
            name,
            depth,
            is_dir: false,
            deleted: true,
        });
    }
}

/// Returns a flat list of all files (non-directories) under `root` using
/// `ignore::WalkBuilder`. Used to populate the search index.
pub fn collect_all_files(root: &Path, show_hidden: bool, ignore_gitignore: bool) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(!show_hidden)
        .git_ignore(!ignore_gitignore)
        .git_global(!ignore_gitignore)
        .git_exclude(!ignore_gitignore)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| !t.is_dir()).unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("tv_tree_{}_{}_{}", name, std::process::id(), n));
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
    fn dirs_before_files_alphabetical() {
        let root = dir_tree();
        // Only root is expanded, so dir_a's children (c.txt) are not shown.
        let expanded = HashSet::from([root.clone()]);
        let nodes = build_visible(&root, &expanded, false, true, &HashSet::new());
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, vec!["dir_a", "dir_b", "a.txt", "b.txt"]);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn collapsed_dir_hides_children() {
        let root = dir_tree();
        let expanded = HashSet::new();
        let nodes = build_visible(&root, &expanded, false, true, &HashSet::new());
        // Root-level entries (dirs + files) all appear at depth 0;
        // nothing is recursed into because expanded is empty.
        assert!(nodes.iter().all(|n| n.depth == 0));
        assert_eq!(nodes.len(), 4, "all root entries visible: {:?}", nodes);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn expanded_dir_shows_children_at_depth_1() {
        let root = dir_tree();
        let expanded = HashSet::from([root.clone(), root.join("dir_a")]);
        let nodes = build_visible(&root, &expanded, false, true, &HashSet::new());
        let c = nodes.iter().find(|n| n.name == "c.txt").unwrap();
        assert_eq!(c.depth, 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn show_hidden_reveals_dotfiles() {
        let root = dir_tree();
        let expanded = HashSet::from([root.clone()]);
        let nodes = build_visible(&root, &expanded, true, true, &HashSet::new());
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&".hidden_file"));
        assert!(names.contains(&".hidden_dir"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn hide_hidden_omits_dotfiles() {
        let root = dir_tree();
        let expanded = HashSet::from([root.clone()]);
        let nodes = build_visible(&root, &expanded, false, true, &HashSet::new());
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
        let nodes = build_visible(&root, &expanded, false, true, &deleted);

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
        let nodes = build_visible(&root, &HashSet::new(), false, true, &HashSet::new());
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

        // ignore_gitignore = false → use gitignore → ignored.log hidden.
        let nodes = build_visible(&root, &expanded, false, false, &HashSet::new());
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(!names.contains(&"ignored.log"));

        // ignore_gitignore = true → ignore gitignore → ignored.log visible.
        let nodes = build_visible(&root, &expanded, false, true, &HashSet::new());
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert!(names.contains(&"ignored.log"));
        fs::remove_dir_all(&root).ok();
    }
}
