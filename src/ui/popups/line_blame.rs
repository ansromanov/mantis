//! Single-line git-blame popup for the content-pane active line.
//!
//! `draw_line_blame` renders a compact bordered overlay showing the short commit
//! hash, author, relative date, and commit subject for the line at
//! `app.active_line`. It is triggered by the `blame_line` keybinding (`B`) and
//! is a no-op on diffs, untracked files, or when the blame data is empty.
//! Reuses `git::file_blame()` — the same call that powers the blame column and
//! the visual-line blame panel — so the blame cache keeps it efficient.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

/// Draws a small popup displaying git blame info for the active content line.
/// A no-op when `show_line_blame` is `false`, when viewing a diff, or when no
/// file is open.
pub(crate) fn draw_line_blame(f: &mut Frame, app: &App, area: Rect) {
    if !app.show_line_blame || app.is_diff {
        return;
    }
    let Some(ref path) = app.current_file else {
        return;
    };

    let theme = &app.theme;
    let phys = app.display_to_physical(app.active_line);
    let lineno = phys + 1;

    // Fetch blame data (cached internally by git::file_blame).
    let blame_lines: Vec<crate::git::BlameLine> = crate::git::file_blame(&app.root, path);

    let blame = blame_lines.iter().find(|b| b.line_no == lineno as u32);

    let popup = centered_rect(60, 12, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(format!(" Blame: line {lineno} — Esc or B to close "))
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));
    let inner = block.inner(popup);
    f.render_widget(&block, popup);

    let dim = Style::default().fg(theme.dim);
    let accent = Style::default().fg(theme.accent);
    let accent_alt = Style::default().fg(theme.accent_alt);
    let text = Style::default().fg(theme.text);

    let rows: Vec<Line> = if let Some(b) = blame {
        vec![
            Line::from(vec![
                Span::styled("Hash:   ", dim),
                Span::styled(b.short_hash.clone(), accent_alt),
            ]),
            Line::from(vec![
                Span::styled("Author: ", dim),
                Span::styled(b.author.clone(), accent),
            ]),
            Line::from(vec![
                Span::styled("Date:   ", dim),
                Span::styled(b.date_relative.clone(), text),
            ]),
            Line::from(vec![
                Span::styled("Msg:    ", dim),
                Span::styled(b.subject.clone(), text),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled(
            " No blame — file is untracked or not in a git repo.",
            dim,
        ))]
    };

    f.render_widget(Paragraph::new(rows), inner);
}

#[cfg(test)]
#[path = "line_blame_test.rs"]
mod line_blame_tests;
