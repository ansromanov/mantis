//! Shared free functions used across `app` submodules.
//!
//! Extracted from `mod.rs` to stay under the 700-line limit. Re-exported from
//! `super::mod.rs` so callers continue to use `super::<fn>` or `crate::app::<fn>`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use ratatui::layout::Rect;

use crate::git::GitStatus;
use crate::theme::Theme;

/// Builds the set of absolute paths that should appear as ghost (deleted) nodes
/// in the tree. Only files that are absent from the working tree are included.
pub(crate) fn deleted_set(map: &HashMap<PathBuf, GitStatus>, enabled: bool) -> HashSet<PathBuf> {
    if !enabled {
        return HashSet::new();
    }
    map.iter()
        .filter(|(path, &status)| status == GitStatus::Deleted && !path.exists())
        .map(|(path, _)| path.clone())
        .collect()
}

/// Returns `true` when `(col, row)` lies within the given `Rect`.
pub(crate) fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && col < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

/// Colors a unified-diff line by its leading marker.
pub(crate) fn diff_line_style(line: &str, theme: &Theme) -> ratatui::style::Style {
    use ratatui::style::{Modifier, Style};
    if line.starts_with("@@") {
        Style::default().fg(theme.accent)
    } else if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(theme.dim).add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(theme.diff_add)
    } else if line.starts_with('-') {
        Style::default().fg(theme.diff_del)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(theme.dim)
    } else {
        Style::default()
    }
}

#[cfg(test)]
#[path = "util_test.rs"]
mod tests;
