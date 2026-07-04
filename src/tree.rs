//! The file tree: a flat `Vec<TreeNode>` built from the filesystem.
//!
//! Instead of a nested tree, the view is modeled as a flat vector where each
//! `TreeNode` carries an explicit `depth` for indentation - far simpler for
//! rendering and mouse hit-testing. `build_visible` walks the root with
//! `ignore::WalkBuilder` (honoring `.gitignore` and hidden-file settings),
//! descending only into directories listed in the `expanded` set and appending
//! git-tracked-but-deleted paths as ghost nodes. `collect_all_files` enumerates
//! every file for the search index. The walk also reports an error count so the
//! UI can surface unreadable directories without aborting the build.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ignore::{DirEntry, WalkBuilder};

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
/// The second element counts walk errors (permission-denied, broken symlinks, etc.).
pub fn build_visible(
    root: &Path,
    expanded: &HashSet<PathBuf>,
    show_hidden: bool,
    ignore_gitignore: bool,
    deleted_files: &HashSet<PathBuf>,
) -> (Vec<TreeNode>, usize) {
    // A single walk over the whole tree, gated by `filter_entry` to only
    // descend into expanded directories. This builds the gitignore chain
    // once instead of re-discovering every ancestor's gitignore per
    // directory (which made the old per-directory-WalkBuilder approach
    // quadratic in tree depth).
    let expanded_for_filter = expanded.clone();
    let mut children: HashMap<PathBuf, Vec<DirEntry>> = HashMap::new();
    let mut error_count = 0usize;

    for result in WalkBuilder::new(root)
        .hidden(!show_hidden)
        .git_ignore(!ignore_gitignore)
        .git_global(!ignore_gitignore)
        .git_exclude(!ignore_gitignore)
        .filter_entry(move |entry| {
            entry.depth() <= 1
                || entry
                    .path()
                    .parent()
                    .is_some_and(|p| expanded_for_filter.contains(p))
        })
        .build()
    {
        match result {
            Ok(e) if e.depth() >= 1 => {
                if let Some(parent) = e.path().parent() {
                    children.entry(parent.to_path_buf()).or_default().push(e);
                }
            }
            Ok(_) => {}
            Err(_) => error_count += 1,
        }
    }

    let mut nodes = Vec::new();
    append_dir(root, 0, &children, expanded, deleted_files, &mut nodes);
    (nodes, error_count)
}

/// Recursive helper for `build_visible`. Sorts one directory's entries
/// (dirs before files, then by extension then name) from the pre-collected
/// `children` map, and recurses into expanded directories.
fn append_dir(
    dir: &Path,
    depth: usize,
    children: &HashMap<PathBuf, Vec<DirEntry>>,
    expanded: &HashSet<PathBuf>,
    deleted_files: &HashSet<PathBuf>,
    out: &mut Vec<TreeNode>,
) {
    let mut entries: Vec<&DirEntry> = match children.get(dir) {
        Some(entries) => entries.iter().collect(),
        None => Vec::new(),
    };

    entries.sort_by(|a, b| {
        let ad = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let bd = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        bd.cmp(&ad).then_with(|| {
            let a_name = a.file_name();
            let b_name = b.file_name();
            let a_ext = Path::new(a_name)
                .extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_default();
            let b_ext = Path::new(b_name)
                .extension()
                .map(|e| e.to_string_lossy())
                .unwrap_or_default();
            a_ext.cmp(&b_ext).then_with(|| a_name.cmp(b_name))
        })
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
            append_dir(&path, depth + 1, children, expanded, deleted_files, out);
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

/// Returns a flat list of all directories (non-root) under `root` using
/// `ignore::WalkBuilder`. Used by `expand_all` to populate the expanded set.
pub fn collect_all_dirs(root: &Path, show_hidden: bool, ignore_gitignore: bool) -> Vec<PathBuf> {
    WalkBuilder::new(root)
        .hidden(!show_hidden)
        .git_ignore(!ignore_gitignore)
        .git_global(!ignore_gitignore)
        .git_exclude(!ignore_gitignore)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.depth() > 0 && e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect()
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
#[path = "tree_test.rs"]
mod tests;
