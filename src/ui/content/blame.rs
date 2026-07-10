//! Blame-annotation rendering for the file-view lane.
//!
//! Two blame display modes, both drawn inside the content pane instead of the
//! tree pane:
//!
//! * **Full-file blame** (`show_blame`): a vertical strip on the left of the
//!   content area showing per-line commit info (hash, author, date), sharing
//!   `content_scroll` so annotations stay 1:1 with visible file lines.
//! * **Line-blame bar** (`show_line_blame`): a 2-row bar at the bottom of the
//!   content pane showing blame for the current `active_line` (hash, author,
//!   date, subject).
//!
//! Both are inactive when the file isn't in a git repo or blame data is empty.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;
use crate::git::BlameLine;

/// Width of the blame annotation strip when full-file blame is active.
/// At least 25 characters, at most 40, roughly 30% of the available pane.
pub(super) fn blame_strip_width(inner_w: u16) -> u16 {
    let pct = (inner_w * 30 / 100).clamp(25, 40);
    pct.min(inner_w.saturating_sub(10))
}

/// Renders the blame annotation strip: a column showing short hash, author, and
/// relative date for each visible file line, sharing `content_scroll` so the
/// annotations stay in lockstep with the file content rendered alongside.
pub(crate) fn draw_blame_annotations(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    blame_lines: &[BlameLine],
) {
    if area.width < 10 || blame_lines.is_empty() {
        return;
    }

    let theme = &app.theme;
    let scroll = app.content_scroll;
    let view_height = area.height as usize;
    let total_display = app.display_line_count();
    let end = (scroll + view_height).min(total_display);

    let dim = Style::default().fg(theme.dim);
    let accent = Style::default().fg(theme.accent);
    let text = Style::default().fg(theme.text);

    let avail = (area.width as usize).saturating_sub(1);
    let hash_w = 7usize;
    let date_w = 12usize.min(avail.saturating_sub(hash_w + 2));
    let author_w = avail.saturating_sub(hash_w + 1 + date_w + 1);
    if author_w == 0 {
        return;
    }

    let mut items: Vec<Line> = Vec::with_capacity(end.saturating_sub(scroll));
    for display_line in scroll..end {
        let phys = app.display_to_physical(display_line);
        let is_active = display_line == app.active_line && app.has_text_cursor();

        let bl = blame_lines.get(phys);

        let hash_style = if is_active { accent } else { dim };
        let author_style = if is_active { text } else { accent };
        let date_style = if is_active { text } else { dim };

        let Some(b) = bl else {
            items.push(Line::from(""));
            continue;
        };

        let hash_s = format!("{:<7}", b.short_hash);
        let author_trunc: String = b.author.chars().take(author_w).collect();
        let date_trunc: String = b.date_relative.chars().take(date_w).collect();

        items.push(Line::from(vec![
            Span::styled(hash_s, hash_style),
            Span::styled(
                format!(" {:<width$}", author_trunc, width = author_w),
                author_style,
            ),
            Span::styled(format!(" {:<date_w$}", date_trunc), date_style),
        ]));
    }

    while items.len() < view_height {
        items.push(Line::from(""));
    }

    f.render_widget(Paragraph::new(items), area);
}

/// Draws a 2-line blame info bar at the bottom of the content pane, showing the
/// short commit hash, author, relative date, and subject for the current
/// `app.active_line`. Triggered by `show_line_blame`.
pub(crate) fn draw_bottom_bar_blame(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(ref path) = app.current_file else {
        return;
    };
    let theme = &app.theme;

    let phys = app.display_to_physical(app.active_line);
    let lineno = phys + 1;

    let blame_lines = crate::git::file_blame(&app.root, path);
    let blame = blame_lines.iter().find(|b| b.line_no == lineno as u32);

    let dim = Style::default().fg(theme.dim);
    let sep = Style::default().fg(theme.dim);
    let sep_line = Line::from(Span::styled("─".repeat(area.width as usize), sep));

    let rows: Vec<Line> = if let Some(b) = blame {
        let max_w = area.width as usize;
        let line1 = format!(" {}  {}  {}", b.short_hash, b.author, b.date_relative);
        let line1_trunc: String = line1.chars().take(max_w).collect();
        let line2_trunc: String = b.subject.chars().take(max_w).collect();

        vec![
            Line::from(Span::styled(line1_trunc, Style::default())),
            Line::from(Span::styled(line2_trunc, dim)),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                " No blame data — file is untracked or not in a git repo.",
                dim,
            )),
            Line::from(""),
        ]
    };

    f.render_widget(
        Paragraph::new(sep_line),
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
    );

    let blame_area = Rect {
        x: area.x,
        y: area.y.saturating_add(1),
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    f.render_widget(Paragraph::new(rows), blame_area);
}

#[cfg(test)]
#[path = "blame_test.rs"]
mod tests;
