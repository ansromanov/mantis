use std::collections::HashSet;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
}

pub fn build_visible(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    ignore_gitignore: bool,
) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    collect(root, 0, expanded, show_hidden, ignore_gitignore, &mut nodes);
    nodes
}

fn collect(
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    ignore_gitignore: bool,
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
        });

        if is_dir && expanded.contains(&path) {
            collect(
                &path,
                depth + 1,
                expanded,
                show_hidden,
                ignore_gitignore,
                out,
            );
        }
    }
}

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
