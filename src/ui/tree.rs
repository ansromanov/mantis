//! File-tree panel rendering.
//!
//! `draw_tree` renders the left-hand file tree from `App::nodes`: it draws each
//! visible `TreeNode` with depth-based indentation, expand/collapse arrows for
//! directories, and git-status coloring (new, modified, deleted, ignored) when
//! status is enabled, marking deleted ghost nodes distinctly. The selected row
//! is highlighted, and focus state controls the border style. At the top of the
//! panel a breadcrumb path bar shows the current directory's ancestors (relative
//! to root) with clickable segments. It records `tree_area`, `tree_offset`, and
//! `breadcrumb_areas` back onto `App` so mouse handlers can map a click row to a
//! node index or a breadcrumb segment to a directory. Rendering only - selection
//! and expansion are driven by the navigation handlers.

use std::path::PathBuf;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus};
use crate::git::GitStatus;

/// Renders the file tree panel with a breadcrumb path bar at the top. Iterates
/// `app.nodes`, drawing indentation, expand/collapse arrows, and git-status
/// coloring. Records `tree_area`, `tree_offset`, and `breadcrumb_areas` for
/// mouse hit-testing.
pub(super) fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Tree)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };
    // `border_style`, `focused`, and `git_suffix` / `title` are Copy values or
    // owned Strings, so the short immutable borrows of `app` for them are done.
    // Defer `let theme = &app.theme` until after breadcrumb rendering, which
    // needs a mutable borrow of `app`.

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

    let inner = block.inner(area);
    f.render_widget(block, area);

    // ── Breadcrumb ──────────────────────────────────────────────────────
    app.breadcrumb_areas.clear();
    let breadcrumb_segments = compute_breadcrumb(app);
    // Only reserve a row for the breadcrumb when the inner width is wide enough
    // for render_breadcrumb to actually draw something (it early-returns at < 3).
    let has_breadcrumb = !breadcrumb_segments.is_empty() && inner.width >= 3;

    let list_area = if has_breadcrumb {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(0)])
            .split(inner);
        render_breadcrumb(f, app, chunks[0], &breadcrumb_segments);
        chunks[1]
    } else {
        inner
    };

    // ── Tree list ───────────────────────────────────────────────────────
    let theme = &app.theme;
    let view_height = list_area.height.max(1) as usize;
    let n = app.nodes.len();

    let offset = if app.tree_independent_scroll {
        let max_scroll = n.saturating_sub(view_height);
        app.tree_scroll = app.tree_scroll.min(max_scroll);
        app.tree_scroll
    } else {
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

    // Precompute last index at each depth for indent guide continuation checks.
    let mut last_at_depth: Vec<usize> = Vec::new();
    for (i, node) in app.nodes.iter().enumerate() {
        if node.depth >= last_at_depth.len() {
            last_at_depth.resize(node.depth + 1, 0);
        }
        last_at_depth[node.depth] = i;
    }

    let guide_style = Style::default().fg(theme.dim).add_modifier(Modifier::DIM);
    let end = (offset + view_height).min(n);
    let items: Vec<ListItem> = app.nodes[offset..end]
        .iter()
        .enumerate()
        .map(|(pos, node)| {
            let global_i = offset + pos;
            let (color, bold) = git_status_style(node, app, theme);
            let name_style = Style::default().fg(color).add_modifier(bold);

            let mut spans: Vec<Span> = Vec::new();
            if app.indent_guides {
                for lvl in 0..node.depth {
                    if last_at_depth.get(lvl).copied().unwrap_or(0) > global_i {
                        spans.push(Span::styled("│  ", guide_style));
                    } else {
                        spans.push(Span::styled("   ", guide_style));
                    }
                }
            } else {
                let indent = "  ".repeat(node.depth);
                spans.push(Span::styled(indent, name_style));
            }

            let arrow = if node.is_dir {
                if app.expanded.contains(&node.path) {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            spans.push(Span::styled(arrow, name_style));
            spans.push(Span::styled(node.name.clone(), name_style));

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD),
    );

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

    f.render_stateful_widget(list, list_area, &mut state);

    // ── Record geometry ─────────────────────────────────────────────────
    app.tree_area = list_area;
    app.tree_offset = offset + state.offset();
    app.tree_scroll = offset + state.offset();
}

/// Computes breadcrumb path segments from the selected tree node to root.
/// Returns a list of (label, target_directory_path) pairs ordered root-first.
/// Always includes at least the root segment when a node is selected; returns
/// empty only when there are no nodes or the path cannot be relativized to root.
fn compute_breadcrumb(app: &App) -> Vec<(String, PathBuf)> {
    let Some(node) = app.nodes.get(app.tree_selected) else {
        return Vec::new();
    };

    let dir_path = if node.is_dir {
        node.path.clone()
    } else {
        match node.path.parent() {
            Some(p) => {
                let p = p.to_path_buf();
                if p.as_os_str().is_empty() {
                    // Relative path like "file.rs" has parent "" which means root.
                    app.root.clone()
                } else {
                    p
                }
            }
            None => return Vec::new(),
        }
    };

    let root_label = app
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "/".to_string());

    let mut segments = vec![(root_label, app.root.clone())];

    if dir_path == app.root {
        return segments;
    }

    let Ok(relative) = dir_path.strip_prefix(&app.root) else {
        return Vec::new();
    };

    let mut cumulative = app.root.clone();
    for component in relative.components() {
        cumulative.push(component.as_os_str());
        segments.push((
            component.as_os_str().to_string_lossy().to_string(),
            cumulative.clone(),
        ));
    }

    segments
}

/// Renders the breadcrumb path bar as a styled line of clickable segments with
/// " / " separators. Truncates middle segments with "…" when the path is wider
/// than the available area. Records each visible segment's `Rect` onto
/// `app.breadcrumb_areas` for mouse hit-testing.
fn render_breadcrumb(f: &mut Frame, app: &mut App, area: Rect, segments: &[(String, PathBuf)]) {
    let theme = &app.theme;
    let avail = area.width as usize;
    if avail < 3 || segments.is_empty() {
        return;
    }

    let sep = " / ";
    let sep_len = sep.len();
    let names_len: usize = segments
        .iter()
        .map(|(n, _)| UnicodeWidthStr::width(n.as_str()))
        .sum();
    let total_len = names_len + segments.len().saturating_sub(1) * sep_len;

    // Pick which segment indices to show (indices into `segments`).
    let show_indices: Vec<usize> = if total_len <= avail {
        (0..segments.len()).collect()
    } else {
        truncate_segments(segments, avail, sep_len)
    };

    let dim_style = Style::default().fg(theme.dim).bg(theme.breadcrumb_bg);
    let fg_style = Style::default()
        .fg(theme.breadcrumb_fg)
        .bg(theme.breadcrumb_bg);
    let last_style = fg_style.add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span> = Vec::new();
    let mut col = area.x;

    for (pos, &idx) in show_indices.iter().enumerate() {
        if idx >= segments.len() {
            // Out-of-bounds sentinel pushed by truncate_segments to mark the
            // position of the "…" ellipsis between kept and dropped segments.
            // "…" is a single terminal cell despite being 3 UTF-8 bytes.
            spans.push(Span::styled("…", dim_style));
            col += 1;
            continue;
        }

        // Separator before each segment except the first.
        if pos > 0 {
            spans.push(Span::styled(sep, dim_style));
            col += sep_len as u16;
        }

        let is_last = pos == show_indices.len() - 1;
        let style = if is_last { last_style } else { fg_style };
        let text = &segments[idx].0;
        spans.push(Span::styled(text.clone(), style));

        let text_w = UnicodeWidthStr::width(text.as_str()) as u16;
        // Record clickable area for this segment.
        let rect = Rect {
            x: col,
            y: area.y,
            width: text_w,
            height: 1,
        };
        app.breadcrumb_areas.push((segments[idx].1.clone(), rect));
        col += text_w;
    }

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(theme.breadcrumb_bg));
    f.render_widget(para, area);
}

/// Picks which segment indices to display when the full breadcrumb exceeds
/// available width. Always keeps the first and last segments; fills as many
/// middle segments as fit, inserting a "…" marker for any that are dropped.
fn truncate_segments(segments: &[(String, PathBuf)], avail: usize, sep_len: usize) -> Vec<usize> {
    if segments.len() <= 2 {
        // With only 2 segments (root + one dir), we can't usefully truncate.
        return (0..segments.len()).collect();
    }

    // Start with first and last, then try to fit as many middle as possible.
    let first = &segments[0];
    let last = &segments[segments.len() - 1];
    let mut used = UnicodeWidthStr::width(first.0.as_str())
        + sep_len
        + UnicodeWidthStr::width(last.0.as_str());
    let mut mid_indices: Vec<usize> = Vec::new();

    // Count how many middle segments fit.
    let mut dropped = false;
    for (i, seg) in segments.iter().enumerate().skip(1).take(segments.len() - 2) {
        let addition = sep_len + UnicodeWidthStr::width(seg.0.as_str());
        // Reserve 1 cell for the "…" ellipsis that signals truncation.
        if used + addition < avail {
            mid_indices.push(i);
            used += addition;
        } else {
            dropped = true;
            break;
        }
    }

    let mut result = Vec::new();
    result.push(0);
    if dropped && !mid_indices.is_empty() {
        // If we dropped some but kept others, we need "…" to signal the gap.
        // Remove the last kept middle segment if it doesn't fit with "…".
        // Actually we already reserved 2 chars for the ellipsis above. The middle
        // indices we kept already account for this.
        result.extend(mid_indices);
        // Out-of-bounds sentinel: the renderer treats idx >= segments.len()
        // as an ellipsis placeholder.
        result.push(segments.len());
    } else if dropped && mid_indices.is_empty() {
        // All middle segments dropped; just show first, "…", last.
        result.push(segments.len());
    } else {
        result.extend(mid_indices);
    }
    result.push(segments.len() - 1);

    result
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
