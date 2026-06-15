//! File-tree panel rendering.
//!
//! `draw_tree` renders the left-hand file tree from `App::nodes`: it draws each
//! visible `TreeNode` with depth-based indentation, expand/collapse arrows for
//! directories, and git-status coloring (new, modified, deleted, ignored) when
//! status is enabled, marking deleted ghost nodes distinctly. The selected row
//! is highlighted, and focus state controls the border style. It records
//! `tree_area` and `tree_offset` back onto `App` so mouse handlers can map a
//! click row to a node index. Rendering only - selection and expansion are
//! driven by the navigation handlers.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, Focus};
use crate::git::GitStatus;

/// Renders the file tree panel. Iterates `app.nodes`, drawing indentation,
/// expand/collapse arrows, and git-status coloring. Records `tree_area` and
/// `tree_offset` for mouse hit-testing.
pub(super) fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let focused = matches!(app.focus, Focus::Tree)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dim)
    };

    let git_suffix = if app.git_mode {
        if app.git_mode_flat {
            " [git:flat]"
        } else {
            " [git]"
        }
    } else {
        ""
    };
    let title = format!(
        " {}{} ",
        app.root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string()),
        git_suffix
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let view_height = area.height.saturating_sub(2).max(1) as usize;
    let n = app.nodes.len();

    // Compute the viewport offset up front so we can slice nodes to exactly
    // the visible rows, bounding ListItem allocation to O(view_height) instead
    // of O(n).
    let offset = if app.tree_independent_scroll {
        // Viewport is cursor-independent; clamp so we never scroll past the end.
        let max_scroll = n.saturating_sub(view_height);
        app.tree_scroll = app.tree_scroll.min(max_scroll);
        app.tree_scroll
    } else {
        // Keep the selected row inside the visible window, updating tree_scroll
        // ourselves so ratatui doesn't need to scan the full list to find it.
        let sel = if n > 0 {
            app.tree_selected.min(n - 1)
        } else {
            0
        };
        if sel < app.tree_scroll {
            sel
        } else if sel >= app.tree_scroll + view_height {
            sel + 1 - view_height
        } else {
            app.tree_scroll
        }
    };

    let end = (offset + view_height).min(n);
    let items: Vec<ListItem> = app.nodes[offset..end]
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth);
            let arrow = if node.is_dir {
                if app.expanded.contains(&node.path) {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let (color, bold) = git_status_style(node, app, theme);
            ListItem::new(format!("{}{}{}", indent, arrow, node.name))
                .style(Style::default().fg(color).add_modifier(bold))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD),
    );

    // Items are pre-sliced to [offset..end], so ListState index 0 == node
    // index `offset`. Express the selection as a relative index within the
    // slice; leave offset_mut() at 0 since we've already handled scrolling.
    let mut state = ListState::default();
    if app.tree_independent_scroll {
        if n > 0 && app.tree_selected >= offset && app.tree_selected < end {
            state.select(Some(app.tree_selected - offset));
        }
    } else if n > 0 {
        let sel = app.tree_selected.min(n - 1);
        if sel >= offset && sel < end {
            state.select(Some(sel - offset));
        }
    }

    f.render_stateful_widget(list, area, &mut state);

    // Record the geometry of the rendered list (inside the border) and the
    // scroll offset so mouse clicks can be mapped back to node indices.
    app.tree_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    // state.offset() is relative to the pre-sliced window; add back `offset`
    // to get the absolute node index for mouse hit-testing and next-frame slicing.
    app.tree_offset = offset + state.offset();
    app.tree_scroll = offset + state.offset();
}

/// Returns the foreground color and modifier for a tree node based on its
/// git status and whether it is a directory. Deleted files get `diff_del`,
/// new files `diff_add`, modified files `accent_alt`, ignored files gray.
fn git_status_style(
    node: &crate::tree::TreeNode,
    app: &App,
    theme: &crate::theme::Theme,
) -> (ratatui::style::Color, Modifier) {
    use ratatui::style::Color;
    let dir_bold = if node.is_dir {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };

    if node.deleted {
        return (theme.diff_del, Modifier::empty());
    }
    if app.git_status_enabled {
        match app.git_status_map.get(&node.path) {
            Some(GitStatus::New) => return (theme.diff_add, dir_bold),
            Some(GitStatus::Modified) => return (theme.accent_alt, dir_bold),
            Some(GitStatus::Deleted) => return (theme.diff_del, dir_bold),
            Some(GitStatus::Ignored) => return (Color::DarkGray, dir_bold),
            None => {}
        }
    }
    if node.is_dir {
        (theme.dir, Modifier::BOLD)
    } else {
        (theme.file, Modifier::empty())
    }
}

#[cfg(test)]
#[path = "tree_test.rs"]
mod tests;
