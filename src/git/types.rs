//! Git-related types used throughout `mantis`.
//!
//! These types are always compiled and populated by the built-in git
//! implementation in `core.rs`.

use std::collections::HashMap;
use std::path::PathBuf;

/// Per-file git working-tree status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatus {
    New,
    Modified,
    Deleted,
    Ignored,
}

/// The current HEAD state of the repository.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum GitHead {
    Branch(String),
    #[default]
    Detached,
    Rebase,
    Merge,
}

impl GitHead {
    pub fn display(&self) -> String {
        match self {
            GitHead::Branch(name) => name.clone(),
            GitHead::Detached => "HEAD (detached)".to_string(),
            GitHead::Rebase => "REBASE".to_string(),
            GitHead::Merge => "MERGE".to_string(),
        }
    }
}

/// Rich git repository info: branch/HEAD state, ahead/behind counts, and
/// dirty file counts.
#[derive(Debug, Clone)]
pub struct GitRepoInfo {
    pub head: GitHead,
    pub ahead: usize,
    pub behind: usize,
    /// Total non-ignored changed files.
    pub total_changed: usize,
    /// Files with staged changes.
    pub staged: usize,
    /// Untracked files.
    pub untracked: usize,
}

impl GitRepoInfo {
    pub fn is_dirty(&self) -> bool {
        self.total_changed > 0
    }
}

/// A commit that touched a particular file.
#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: String,
    pub short: String,
    pub date: String,
    pub subject: String,
}

/// Per-line git blame annotation.
#[derive(Clone, Debug)]
pub struct BlameLine {
    #[allow(dead_code)]
    pub commit_hash: String,
    pub short_hash: String,
    pub author: String,
    pub date_relative: String,
    pub line_no: u32,
    /// Subject line of the commit (commit message summary).
    pub subject: String,
}

pub(crate) fn status_priority(s: GitStatus) -> u8 {
    match s {
        GitStatus::Modified => 3,
        GitStatus::New => 2,
        GitStatus::Deleted => 1,
        GitStatus::Ignored => 0,
    }
}

pub(crate) fn set_if_higher(map: &mut HashMap<PathBuf, GitStatus>, key: PathBuf, val: GitStatus) {
    let cur = map.entry(key).or_insert(val);
    if status_priority(val) > status_priority(*cur) {
        *cur = val;
    }
}

#[cfg(test)]
#[path = "types_test.rs"]
mod tests;
