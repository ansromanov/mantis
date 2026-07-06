//! File-tree panel rendering.
//!
//! `draw_tree` renders the left-hand file tree from `App::nodes`: it draws each
//! visible `TreeNode` with depth-based indentation, expand/collapse arrows for
//! directories, and git-status coloring (new, modified, deleted, ignored) when
//! status is enabled, marking deleted ghost nodes distinctly. When in git mode
//! with no changed files, a centered placeholder is rendered instead of the tree:
//! a "+" icon with "Working tree clean" for a clean repo, or "!" with "Not a git
//! repository" when no git info is available. When an inline tree filter
//! (`App::tree_filter`) is active, only nodes whose names match the query (plus
//! their ancestor directories) are rendered. The selected row is highlighted, and
//! focus state controls the border style. At the top of the panel a breadcrumb
//! path bar shows the filesystem ancestors of `app.root` (from `/` down to the
//! root itself) as clickable segments — it represents where the root is, not
//! where the cursor is. It records `tree_area`, `tree_offset`, and
//! `breadcrumb_areas` back onto `App` so mouse handlers can map a click row to a
//! node index or a breadcrumb segment to a directory. Rendering only - selection
//! and expansion are driven by the navigation handlers.

use std::collections::HashSet;
use std::path::PathBuf;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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
pub fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Tree)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };
    // `border_style` and `focused` are Copy values, so the short immutable
    // borrows of `app` for them are done. Defer `let theme = &app.theme` until
    // after breadcrumb rendering, which needs a mutable borrow of `app`.

    let title = format!(" {} ", app.panel_mode_label());

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

    // ── Empty git-mode placeholder ──────────────────────────────────────
    if app.git_mode && app.nodes.is_empty() {
        let is_repo = app.git_info.is_some();
        let (icon, main, detail, color) = if is_repo {
            (
                "+",
                " Working tree clean",
                "No changes to show.",
                app.theme.git_clean,
            )
        } else {
            (
                "!",
                " Not a git repository",
                "No git data available.",
                app.theme.git_dirty,
            )
        };
        let hint = app.keys().label_for_action("git_mode_toggle");
        let hint_line = if hint.is_empty() {
            Line::from("")
        } else {
            Line::from(Span::styled(
                format!("{hint} to exit git mode"),
                Style::default().fg(app.theme.dim),
            ))
        };
        let lines = vec![
            Line::from(vec![
                Span::styled(icon, Style::default().fg(color)),
                Span::styled(main, Style::default().fg(app.theme.text)),
            ]),
            Line::from(Span::styled(detail, Style::default().fg(app.theme.dim))),
            hint_line,
        ];
        let placeholder = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .style(Style::default());
        f.render_widget(placeholder, list_area);
        app.tree_area = list_area;
        app.tree_offset = 0;
        app.tree_visible_indices = None;
        return;
    }

    // ── Tree list ───────────────────────────────────────────────────────
    let theme = &app.theme;
    let view_height = list_area.height.max(1) as usize;
    let total_nodes = app.nodes.len();

    // When an inline tree filter is active, compute which node indices to show:
    // any node whose name contains the query (case-insensitive), plus all
    // ancestor directories of matching nodes so the tree remains navigable.
    // Results are cached on (query, tree_revision) so this only recomputes
    // when the query changes or the tree rebuilds.
    let mut visible_indices: Vec<usize> = (0..total_nodes).collect();
    let mut cache_miss = false;
    if let Some(ref filter) = app.tree_filter {
        if !filter.is_empty() {
            let use_cache = filter.cached.as_ref().and_then(|(q, rev, vis)| {
                if q == &filter.query && *rev == app.tree_revision {
                    Some(vis.clone())
                } else {
                    None
                }
            });
            match use_cache {
                Some(cached) => visible_indices = cached,
                None => {
                    cache_miss = true;
                    let q: String = filter.query.to_lowercase();
                    let matching: HashSet<usize> = (0..total_nodes)
                        // index i is loop-bounded by 0..total_nodes
                        .filter(|&i| app.nodes[i].name.to_lowercase().contains(&q))
                        .collect();
                    let mut include = matching.clone();
                    for &mi in &matching {
                        // index mi is from matching, a subset of 0..total_nodes
                        let match_path = &app.nodes[mi].path;
                        let match_depth = app.nodes[mi].depth;
                        for j in (0..mi).rev() {
                            // index j is loop-bounded by 0..mi
                            let nd = &app.nodes[j];
                            if nd.depth >= match_depth {
                                continue;
                            }
                            if nd.is_dir && match_path.starts_with(&nd.path) {
                                include.insert(j);
                            }
                        }
                    }
                    let mut sorted: Vec<usize> = include.into_iter().collect();
                    sorted.sort_unstable();
                    visible_indices = sorted;
                }
            }
        }
    }

    if cache_miss {
        if let Some(ref mut filter) = app.tree_filter {
            filter.cached = Some((
                filter.query.clone(),
                app.tree_revision,
                visible_indices.clone(),
            ));
        }
    }

    let n = visible_indices.len();

    // Map `tree_selected` (original index) to its position in the filtered view.
    let sel_in_view = visible_indices
        .iter()
        .position(|&i| i == app.tree_selected)
        .unwrap_or(0);

    let max_scroll = n.saturating_sub(view_height);
    app.tree_scroll = app.tree_scroll.min(max_scroll);
    let offset = app.tree_scroll;

    // Indent guide masks depend only on node depths, not on scroll position or
    // filtering, so they're computed once per tree rebuild and cached on
    // `tree_revision` rather than redone on every render.
    let guide_style = Style::default().fg(theme.dim).add_modifier(Modifier::DIM);
    let end = (offset + view_height).min(n);
    if app.indent_guides && total_nodes > 0 {
        let stale = app
            .tree_guide_cache
            .as_ref()
            .map(|(rev, _)| *rev != app.tree_revision)
            .unwrap_or(true);
        if stale {
            let max_depth = app.nodes.iter().map(|nd| nd.depth).max().unwrap_or(0);
            let mut pending = vec![false; max_depth + 1];
            let mut masks: Vec<Vec<bool>> = vec![Vec::new(); total_nodes];
            for i in (0..total_nodes).rev() {
                let d = app.nodes[i].depth; // index i is loop-bounded by 0..total_nodes
                masks[i] = (0..d).map(|lvl| pending[lvl]).collect();
                pending[d] = true;
                pending[(d + 1)..=max_depth].fill(false);
            }
            app.tree_guide_cache = Some((app.tree_revision, masks));
        }
    }

    let items: Vec<ListItem> = visible_indices[offset..end]
        .iter()
        .map(|&global_i| {
            // global_i is from visible_indices, a filtered subset of 0..total_nodes
            let node = &app.nodes[global_i];
            let (color, bold) = git_status_style(node, app, theme);
            let name_style = Style::default().fg(color).add_modifier(bold);

            let mut spans: Vec<Span> = Vec::new();
            if app.indent_guides {
                let mask = app
                    .tree_guide_cache
                    .as_ref()
                    .and_then(|(_, masks)| masks.get(global_i));
                for lvl in 0..node.depth {
                    if mask.is_some_and(|m| m.get(lvl).copied().unwrap_or(false)) {
                        spans.push(Span::styled("│  ", guide_style));
                    } else {
                        spans.push(Span::styled("   ", guide_style));
                    }
                }
            } else {
                let indent = "  ".repeat(node.depth);
                spans.push(Span::styled(indent, name_style));
            }

            let is_open = node.is_dir && app.expanded.contains(&node.path);
            let arrow = if node.is_dir {
                if is_open {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            spans.push(Span::styled(arrow, name_style));

            if app.icons_enabled
                && (!app.icon_map.is_empty()
                    || !app.icon_fallback.is_empty()
                    || !app.icon_dir_open.is_empty()
                    || !app.icon_dir_closed.is_empty())
            {
                let icon = if node.is_dir {
                    if is_open {
                        &app.icon_dir_open
                    } else {
                        &app.icon_dir_closed
                    }
                } else {
                    let ext = node
                        .path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    let key = if ext.is_empty() {
                        node.path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("")
                            .to_lowercase()
                    } else {
                        ext
                    };
                    app.icon_map.get(&key).unwrap_or(&app.icon_fallback)
                };
                if !icon.is_empty() {
                    spans.push(Span::styled(icon.as_str(), name_style));
                    spans.push(Span::styled(" ", name_style));
                }
            }

            // Inline tree-filter match highlighting
            let name_spans = if let Some(ref filter) = app.tree_filter {
                if filter.is_empty() {
                    vec![Span::styled(node.name.clone(), name_style)]
                } else {
                    highlight_matches(&node.name, &filter.query, name_style, theme)
                }
            } else {
                vec![Span::styled(node.name.clone(), name_style)]
            };
            spans.extend(name_spans);

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if n > 0 && sel_in_view >= offset && sel_in_view < end {
        state.select(Some(sel_in_view - offset));
    }

    f.render_stateful_widget(list, list_area, &mut state);

    // ── Record geometry ─────────────────────────────────────────────────
    app.tree_area = list_area;
    app.tree_offset = offset + state.offset();
    app.tree_scroll = offset + state.offset();
    // When no filter is active, set visible_indices to None (identity).
    let has_filter = app.tree_filter.as_ref().is_some_and(|f| !f.is_empty());
    app.tree_visible_indices = if has_filter {
        Some(visible_indices)
    } else {
        None
    };
}

/// Splits `name` into spans, highlighting substrings that match `query`
/// (case-insensitive). Matching segments get a bold accent style.
///
/// All indexing is done in `char` space against the original name's chars, so
/// multibyte (non-ASCII) file names are sliced on character boundaries and
/// never panic. Lowercasing is done per-char to keep a 1:1 alignment between
/// the match buffer and the original characters used to build the spans.
fn highlight_matches(
    name: &str,
    query: &str,
    base_style: Style,
    theme: &crate::theme::Theme,
) -> Vec<Span<'static>> {
    // Original characters used to build the output spans.
    let name_chars: Vec<char> = name.chars().collect();
    // Per-char lowercased buffers kept 1:1 with `name_chars` / query chars.
    let lower_first = |c: char| c.to_lowercase().next().unwrap_or(c);
    let name_lower: Vec<char> = name_chars.iter().map(|&c| lower_first(c)).collect();
    let q_lower: Vec<char> = query.chars().map(lower_first).collect();
    let q_char_len = q_lower.len();
    if q_char_len == 0 || name_lower.len() < q_char_len {
        return vec![Span::styled(name.to_string(), base_style)];
    }

    // Find all match start positions (char indices).
    let mut match_starts = Vec::new();
    let mut i = 0;
    while i + q_char_len <= name_lower.len() {
        if name_lower[i..i + q_char_len] == q_lower[..] {
            match_starts.push(i);
            i += q_char_len;
        } else {
            i += 1;
        }
    }

    if match_starts.is_empty() {
        return vec![Span::styled(name.to_string(), base_style)];
    }

    let highlight_style = base_style
        .fg(theme.accent_alt)
        .add_modifier(ratatui::style::Modifier::BOLD);
    let slice = |range: std::ops::Range<usize>| name_chars[range].iter().collect::<String>();
    let mut spans = Vec::new();
    let mut pos = 0;
    for &ms in &match_starts {
        if ms > pos {
            spans.push(Span::styled(slice(pos..ms), base_style));
        }
        spans.push(Span::styled(slice(ms..ms + q_char_len), highlight_style));
        pos = ms + q_char_len;
    }
    if pos < name_chars.len() {
        spans.push(Span::styled(slice(pos..name_chars.len()), base_style));
    }
    spans
}

/// Computes breadcrumb path segments from the filesystem root (`/`) down
/// through `app.root`. Returns a list of (label, target_directory_path) pairs
/// ordered root-first. Always includes at least the root segment when there are
/// nodes; returns empty only when there are no nodes. The breadcrumb represents
/// the root's location — it never descends into the cursor's subdirectory.
fn compute_breadcrumb(app: &App) -> Vec<(String, PathBuf)> {
    if app.nodes.is_empty() {
        return Vec::new();
    }

    // Build segments from the filesystem root (/) down through app.root.
    // Walk leaf-first, then reverse once so we allocate only one Vec.
    let mut segments: Vec<(String, PathBuf)> = Vec::new();
    let mut current = app.root.as_path();
    loop {
        let label = current
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        segments.push((label, current.to_path_buf()));
        match current.parent() {
            Some(parent) if parent != current => current = parent,
            _ => break,
        }
    }
    segments.reverse();

    segments
}

/// Represents a single item in the compact breadcrumb display.
#[derive(Debug, Clone)]
enum BreadcrumbItem {
    /// Index into the original `segments` slice.
    Real(usize),
    /// The ".." marker standing in for collapsed ancestors.
    /// The `PathBuf` is the target directory that clicking ".." navigates to
    /// (the parent of the first kept real segment).
    ParentUp(PathBuf),
}

/// Renders the breadcrumb path bar as a styled line of clickable segments with
/// " / " separators. Compacts deep paths by collapsing leading ancestors into
/// a single ".." segment when the path is wider than the available area.
/// Records each visible segment's `Rect` onto `app.breadcrumb_areas` for mouse
/// hit-testing.
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
    // The root segment (empty label) renders as "/" plus a single-space
    // separator to the next segment, not the usual " / " — narrower than the
    // generic formula below assumes, so it needs its own width count to avoid
    // triggering compaction earlier than the actual render requires.
    let total_len = if segments[0].0.is_empty() {
        match segments.len() {
            1 => 1,
            n => 2 + names_len + (n - 2) * sep_len,
        }
    } else {
        names_len + segments.len().saturating_sub(1) * sep_len
    };

    // Pick which items to show (indices or compact markers).
    let items: Vec<BreadcrumbItem> = if total_len <= avail {
        (0..segments.len()).map(BreadcrumbItem::Real).collect()
    } else {
        compact_segments(segments, avail, sep_len)
    };

    let dim_style = Style::default().fg(theme.dim).bg(theme.breadcrumb_bg);
    let fg_style = Style::default()
        .fg(theme.breadcrumb_fg)
        .bg(theme.breadcrumb_bg);
    let last_style = fg_style.add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span> = Vec::new();
    let mut col = area.x;
    let mut after_root = false;

    for (pos, item) in items.iter().enumerate() {
        match item {
            BreadcrumbItem::ParentUp(target) => {
                // ".." marker: collapsed ancestors, shown first (no separator before).
                // Style it dim to distinguish from real segments.
                spans.push(Span::styled("..", dim_style));
                let rect = Rect {
                    x: col,
                    y: area.y,
                    width: 2,
                    height: 1,
                };
                app.breadcrumb_areas.push((target.clone(), rect));
                col += 2;
            }
            BreadcrumbItem::Real(idx) => {
                let text = &segments[*idx].0;
                if text.is_empty() {
                    // Root segment (empty label since the filesystem root has no
                    // file_name) — render "/" without a trailing separator, and
                    // flag after_root so the next segment skips the full separator.
                    spans.push(Span::styled("/", dim_style));
                    let rect = Rect {
                        x: col,
                        y: area.y,
                        width: 1,
                        height: 1,
                    };
                    app.breadcrumb_areas.push((segments[*idx].1.clone(), rect));
                    col += 1;
                    after_root = true;
                } else {
                    if after_root {
                        // After the root segment: use a space instead of " / " to
                        // avoid doubling the slash (root already provides "/").
                        spans.push(Span::styled(" ", dim_style));
                        col += 1;
                        after_root = false;
                    } else if pos > 0 {
                        spans.push(Span::styled(sep, dim_style));
                        col += sep_len as u16;
                    }

                    let is_last = pos == items.len() - 1;
                    let style = if is_last { last_style } else { fg_style };
                    spans.push(Span::styled(text.clone(), style));

                    let text_w = UnicodeWidthStr::width(text.as_str()) as u16;
                    let rect = Rect {
                        x: col,
                        y: area.y,
                        width: text_w,
                        height: 1,
                    };
                    app.breadcrumb_areas.push((segments[*idx].1.clone(), rect));
                    col += text_w;
                }
            }
        }
    }

    let line = Line::from(spans);
    let para = Paragraph::new(line).style(Style::default().bg(theme.breadcrumb_bg));
    f.render_widget(para, area);
}

/// Decides which breadcrumb items to display when the full path exceeds
/// available width. Replaces the leading run of ancestor segments with a
/// single ".." marker (whose click target is the parent of the first kept
/// segment), showing as many trailing segments as fit. The current directory
/// (last segment) is always visible.
fn compact_segments(
    segments: &[(String, PathBuf)],
    avail: usize,
    sep_len: usize,
) -> Vec<BreadcrumbItem> {
    if segments.len() <= 2 {
        return (0..segments.len()).map(BreadcrumbItem::Real).collect();
    }

    let seg_widths: Vec<usize> = segments
        .iter()
        .map(|(n, _)| UnicodeWidthStr::width(n.as_str()))
        .collect();

    // Try keeping as many suffix segments as possible.
    // `first_kept` is the index of the first segment we keep (non-compacted);
    // smaller first_kept → more segments shown. We iterate forward so the
    // first fit keeps the most segments.
    for first_kept in 1..segments.len() {
        let kept_count = segments.len() - first_kept;
        let names: usize = seg_widths[first_kept..].iter().sum();
        let seps = kept_count.saturating_sub(1) * sep_len;
        let compact_prefix = 2 + sep_len;
        let total = compact_prefix + names + seps;

        if total <= avail {
            let target = segments[first_kept]
                .1
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| segments[0].1.clone());
            let mut result = Vec::new();
            result.push(BreadcrumbItem::ParentUp(target));
            for i in first_kept..segments.len() {
                result.push(BreadcrumbItem::Real(i));
            }
            return result;
        }
    }

    // Fallback: only ".." + last segment fit (always at least 2).
    let target = segments[segments.len() - 1]
        .1
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| segments[0].1.clone());
    vec![
        BreadcrumbItem::ParentUp(target),
        BreadcrumbItem::Real(segments.len() - 1),
    ]
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
