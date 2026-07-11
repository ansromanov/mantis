//! Supporting types used by the central `App` struct: focus, diff mode, status
//! messages, and cache keys.
//!
//! Extracted from `mod.rs` to stay under the 700-line project limit. Re-exported
//! from `super::mod.rs` so callers continue to use `crate::app::Focus`,
//! `crate::app::DiffMode`, etc.

use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Which panel is currently focused.
#[derive(Debug, PartialEq)]
pub enum Focus {
    /// The file tree panel on the left.
    Tree,
    /// The file content / diff panel on the right.
    Content,
}

/// Which git diff view is active in the content pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffMode {
    /// All changes vs HEAD (`git diff HEAD`) — the default.
    #[default]
    All,
    /// Only staged changes (`git diff --cached`).
    Staged,
    /// Only unstaged changes (`git diff`).
    Unstaged,
}

impl DiffMode {
    /// Cycles through All -> Staged -> Unstaged -> All.
    pub fn next(self) -> Self {
        match self {
            DiffMode::All => DiffMode::Staged,
            DiffMode::Staged => DiffMode::Unstaged,
            DiffMode::Unstaged => DiffMode::All,
        }
    }

    /// Short label used in the content title badge.
    pub fn label(self) -> &'static str {
        match self {
            DiffMode::All => "all",
            DiffMode::Staged => "staged",
            DiffMode::Unstaged => "unstaged",
        }
    }
}

/// A transient status message with a timestamp so it can auto-expire.
#[derive(Debug)]
pub struct StatusMessage {
    pub text: String,
    pub set_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>, now: Instant) -> Self {
        Self {
            text: text.into(),
            set_at: now,
        }
    }

    /// Returns `true` when the message has been alive for at least `ttl`.
    pub fn expired(&self, ttl: Duration) -> bool {
        self.set_at.elapsed() >= ttl
    }
}

/// Immutable snapshot of a file at a specific git revision. When active, the
/// content pane shows the historical file content (highlighted, foldable,
/// searchable) instead of the working-tree file or a diff. The buffer is
/// read-only and watcher reloads must not replace it.
#[derive(Debug, Clone)]
pub struct FileAtRevision {
    /// Short hash for display in the title bar.
    pub short: String,
    /// Saved diff state so the toggle can restore the diff without re-fetching.
    pub saved_diff: Option<SavedDiffState>,
}

/// Saved diff state for toggling between diff and file-at-revision views.
/// Stored in [`FileAtRevision::saved_diff`] so the toggle back to diff is
/// instant.
#[derive(Debug, Clone)]
pub struct SavedDiffState {
    pub content: Vec<String>,
    pub highlighted: Vec<Vec<(ratatui::style::Style, String)>>,
    pub diff_rows: Vec<crate::diff::DiffRow>,
    pub content_title: String,
    pub content_scroll: usize,
    pub active_line: usize,
    pub side_by_side: bool,
}

/// A keypress dispatched to `on_keypress` subscribers (protocol 3+), waiting
/// to see whether any of them claims it via a `key_handled` action before
/// `deadline`. If claimed in time, the key is swallowed — no built-in
/// binding fires for it; if the deadline passes first, normal-mode handling
/// falls through exactly as it would for a plugin that never replies.
#[derive(Debug)]
pub(crate) struct PendingKeypress {
    pub(crate) key: crossterm::event::KeyEvent,
    pub(crate) deadline: Instant,
}

/// Cache key for syntax-highlighted visible window. When all fields match the
/// current rendering state the cached highlight spans can be reused without
/// re-invoking syntect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HighlightCacheKey {
    pub path: PathBuf,
    pub scroll: usize,
    pub visible_end: usize,
    pub theme: String,
    pub word_wrap: bool,
}

pub(crate) type HighlightCacheValue = Vec<Vec<(ratatui::style::Style, String)>>;

#[cfg(test)]
#[path = "types_test.rs"]
mod tests;
