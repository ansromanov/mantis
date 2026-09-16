//! The workspace list: every open project tab as a row at the top of the left panel.
//!
//! Only drawn (by `ui::draw_area`, on behalf of `ui::draw_workspace`) when more
//! than one tab is open, or while the "open project as new tab" prompt is
//! active -- a single-tab session looks exactly like it did before tabs
//! existed. The list is a bordered ` Workspaces ` block stacked above the file
//! tree and styled like tree rows: each row shows the project root's shortest
//! unique name plus its git branch/changed-count badge, the active workspace is
//! highlighted with the selection style, and every row ends in a `×` close
//! glyph. When there are more workspaces than rows, the list scrolls vertically
//! and shows `▴`/`▾` affordances on its top and bottom borders. Row ranges are
//! recomputed identically by the renderer and [`hit_test`] so mouse clicks land
//! on the same workspace or affordance the list visually shows. Owns
//! [`WorkspaceList`] (the label snapshot built before the active `App` is
//! mutably borrowed for drawing), [`TabHit`], and the height/visibility math.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use unicode_width::UnicodeWidthStr;

use crate::workspace::Tabs;

/// What a mouse click on the workspace list landed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabHit {
    Switch(usize),
    Close(usize),
    ScrollUp,
    ScrollDown,
}

/// Rows assumed visible before the list has been drawn once.
pub(crate) const DEFAULT_VISIBLE_ROWS: u16 = 5;
/// Columns taken by the leading space and the two-cell active marker.
const ROW_PREFIX_WIDTH: usize = 3;
/// Columns taken by the trailing ` × ` close region.
const CLOSE_WIDTH: u16 = 3;

/// One workspace row's untruncated label and git badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceRow {
    pub label: String,
    pub badge: String,
}

/// Snapshot of everything the list needs to render, taken from [`Tabs`] so the
/// active `App` can then be borrowed mutably for the rest of the frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceList {
    pub rows: Vec<WorkspaceRow>,
    pub active: usize,
    pub first_visible: usize,
    pub prompt: Option<String>,
}

impl WorkspaceList {
    /// Builds the row labels and badges for every open tab.
    pub(crate) fn from_tabs(tabs: &Tabs) -> Self {
        let rows = (0..tabs.apps.len())
            .map(|index| WorkspaceRow {
                label: unique_tab_label(tabs, index),
                badge: tab_badge(&tabs.apps[index]),
            })
            .collect();
        WorkspaceList {
            rows,
            active: tabs.active,
            first_visible: tabs.first_visible,
            prompt: tabs.new_tab_prompt.clone(),
        }
    }

    /// Content rows needed to show every workspace plus the prompt, if open.
    fn wanted_rows(&self) -> u16 {
        let prompt = u16::from(self.prompt.is_some());
        (self.rows.len() as u16).saturating_add(prompt)
    }

    /// Total block height (borders included) for a left column `column_height` tall.
    /// The list takes at most a third of the column so the file tree keeps most of it.
    pub(crate) fn height_for(&self, column_height: u16) -> u16 {
        let max_rows = (column_height / 3).saturating_sub(2).max(1);
        self.wanted_rows().min(max_rows).saturating_add(2)
    }
}

/// Number of workspace rows that fit inside a list block of `area`.
pub(crate) fn visible_rows(tabs_prompt_open: bool, area: Rect) -> u16 {
    area.height
        .saturating_sub(2)
        .saturating_sub(u16::from(tabs_prompt_open))
}

fn inactive_tab_style(theme: &crate::theme::Theme) -> Style {
    Style::default().fg(theme.text)
}

fn close_tab_style(theme: &crate::theme::Theme) -> Style {
    Style::default().fg(theme.dim)
}

/// Truncates `text` to `max_len` characters, ending in `…` when shortened.
fn truncate(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        return text.to_string();
    }
    if max_len == 0 {
        return String::new();
    }
    let truncated: String = text.chars().take(max_len - 1).collect();
    format!("{truncated}…")
}

/// The display label for a tab: its root directory's file name, truncated.
#[cfg(test)]
fn tab_label_with_max(app: &crate::app::App, max_len: usize) -> String {
    let name = app
        .root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| app.root.display().to_string());
    truncate(&name, max_len)
}

/// Returns whether `target` is rendered when the list starts at `first_visible`
/// and has `rows` content rows.
#[cfg(test)]
pub(crate) fn is_tab_visible(first_visible: usize, target: usize, rows: u16) -> bool {
    target >= first_visible && target < first_visible + usize::from(rows.max(1))
}

/// The first visible row after clamping `first_visible` so that no rows are
/// wasted past the end of the list and `active` stays in view.
pub(crate) fn scroll_offset(first_visible: usize, active: usize, len: usize, rows: u16) -> usize {
    let rows = usize::from(rows.max(1));
    let first = first_visible.min(len.saturating_sub(rows));
    if active < first {
        active
    } else if active >= first + rows {
        active + 1 - rows
    } else {
        first
    }
}

/// Draws the workspace list into `area` and returns it for hit-testing.
pub(crate) fn draw_workspace_list(
    f: &mut Frame,
    theme: &crate::theme::Theme,
    list: &WorkspaceList,
    area: Rect,
) -> Rect {
    let rows = visible_rows(list.prompt.is_some(), area);
    let first_visible = scroll_offset(list.first_visible, list.active, list.rows.len(), rows);
    let hidden_above = first_visible > 0;
    let hidden_below = first_visible + usize::from(rows) < list.rows.len();

    let border = Style::default().fg(theme.dim);
    let affordance = Style::default()
        .fg(theme.accent_alt)
        .add_modifier(Modifier::BOLD);
    let mut block = Block::default()
        .title(" Workspaces ")
        .borders(Borders::ALL)
        .border_style(border);
    if hidden_above {
        block = block.title(Line::from(Span::styled(" ▴ ", affordance)).right_aligned());
    }
    if hidden_below {
        block = block.title_bottom(Line::from(Span::styled(" ▾ ", affordance)).right_aligned());
    }
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return area;
    }

    let width = usize::from(inner.width);
    for (offset, index) in (first_visible..list.rows.len())
        .take(usize::from(rows))
        .enumerate()
    {
        let row = &list.rows[index];
        let active = index == list.active;
        let style = if active {
            theme.selection_style().add_modifier(Modifier::BOLD)
        } else {
            inactive_tab_style(theme)
        };
        let marker = if active { "▸ " } else { "  " };
        let text_width = width.saturating_sub(ROW_PREFIX_WIDTH + usize::from(CLOSE_WIDTH));
        // Preserve project identity first: truncate the badge only when the
        // row cannot show it in full, then use the remaining space for the
        // project label instead of dropping the badge altogether.
        let badge = truncate(&row.badge, text_width);
        let label = truncate(&row.label, text_width.saturating_sub(badge.width()));
        let pad = text_width.saturating_sub(label.width() + badge.width());
        let mut spans = vec![
            Span::styled(format!(" {marker}{label}"), style),
            Span::styled(badge, style.fg(theme.accent_alt)),
            Span::styled(" ".repeat(pad), style),
        ];
        if width >= ROW_PREFIX_WIDTH + usize::from(CLOSE_WIDTH) {
            spans.push(Span::styled(" ", style));
            spans.push(Span::styled("×", style.patch(close_tab_style(theme))));
            spans.push(Span::styled(" ", style));
        }
        let row_area = Rect::new(inner.x, inner.y + offset as u16, inner.width, 1);
        f.render_widget(Paragraph::new(Line::from(spans)), row_area);
    }

    if let Some(query) = &list.prompt {
        let row_area = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!(" open: {query}\u{2588}"),
                Style::default().fg(theme.accent_alt),
            ))),
            row_area,
        );
    }
    area
}

/// Produces the shortest trailing path suffix that uniquely identifies a tab.
fn unique_tab_label(tabs: &Tabs, index: usize) -> String {
    let Some(app) = tabs.apps.get(index) else {
        return String::new();
    };
    let parts = app
        .root
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let suffix = |root: &std::path::Path, count: usize| {
        let parts = root
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        parts[parts.len().saturating_sub(count)..].join("/")
    };
    let mut count = 1;
    while count < parts.len() {
        let candidate = suffix(&app.root, count);
        let unique = tabs.apps.iter().enumerate().all(|(other_index, other)| {
            other_index == index || suffix(&other.root, count) != candidate
        });
        if unique {
            break;
        }
        count += 1;
    }
    suffix(&app.root, count)
}

fn tab_badge(app: &crate::app::App) -> String {
    let Some(info) = &app.git_info else {
        return String::new();
    };
    let mut badge = format!(" ·{}", info.head.display());
    if info.total_changed > 0 {
        badge.push_str(&format!(" ●{}", info.total_changed));
    }
    badge
}

/// Maps a mouse position to the workspace row or affordance it landed on, or
/// `None` when it missed the list, hit a border, or landed on an empty row.
pub(crate) fn hit_test(tabs: &Tabs, area: Rect, col: u16, row: u16) -> Option<TabHit> {
    if tabs.new_tab_prompt.is_some()
        || col < area.x
        || col >= area.right()
        || row < area.y
        || row >= area.bottom()
        || area.height < 3
    {
        return None;
    }
    let rows = visible_rows(false, area);
    let first_visible = scroll_offset(tabs.first_visible, tabs.active, tabs.apps.len(), rows);
    if row == area.y {
        return (first_visible > 0).then_some(TabHit::ScrollUp);
    }
    if row == area.bottom() - 1 {
        let hidden_below = first_visible + usize::from(rows) < tabs.apps.len();
        return hidden_below.then_some(TabHit::ScrollDown);
    }
    if col == area.x || col == area.right() - 1 {
        return None;
    }
    let index = first_visible + usize::from(row - area.y - 1);
    if index >= tabs.apps.len() {
        return None;
    }
    let close_start = area.right().saturating_sub(1 + CLOSE_WIDTH);
    if col >= close_start && area.width >= 2 + ROW_PREFIX_WIDTH as u16 + CLOSE_WIDTH {
        Some(TabHit::Close(index))
    } else {
        Some(TabHit::Switch(index))
    }
}

#[cfg(test)]
#[path = "tabstrip_test.rs"]
mod tests;
