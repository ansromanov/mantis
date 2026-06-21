//! The selection-scoped git-blame panel.
//!
//! `draw_blame_panel` renders blame information for the line range currently
//! selected in visual-line mode. Each row pairs a line with its short commit
//! hash, author, relative date, and line number, drawn in a bordered overlay so
//! the user can see who last touched the highlighted lines. It is a no-op when
//! visual-line mode is inactive. Blame data is gathered through the `git`
//! module's shell-outs; this file only formats and lays out the result, reading
//! the active range and theme from `App`.

use std::collections::HashMap;

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

/// Draws the selection-scoped git-blame panel for the active visual-line range.
/// Each line shows its short commit hash, author, relative date, line number,
/// and content. A no-op when visual-line mode is inactive.
pub(crate) fn draw_blame_panel(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let Some(v) = app.visual_line.as_ref() else {
        return;
    };
    let Some(path) = app.current_file.as_ref() else {
        return;
    };
    let (start, end) = v.range();

    let popup = centered_rect(82, 60, area);
    f.render_widget(Clear, popup);

    let start_no = app.display_to_physical(start) + 1;
    let end_no = app.display_to_physical(end) + 1;
    let block = Block::default()
        .title(format!(
            " Blame: L{start_no}\u{2013}L{end_no} — Esc to close "
        ))
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    // Plugin-provided blame data takes precedence over live git blame.
    let plugin_lines: Option<Vec<String>> = app
        .plugin_blame
        .get(path)
        .filter(|v| !v.is_empty())
        .cloned();
    let git_blame: Vec<crate::git::BlameLine> = if plugin_lines.is_none() {
        #[cfg(feature = "git-core")]
        {
            crate::git::file_blame(&app.root, path)
        }
        #[cfg(not(feature = "git-core"))]
        {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    if plugin_lines.is_none() && git_blame.is_empty() {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                " No blame available — file is untracked or not in a git repo.",
                Style::default().fg(theme.dim),
            ))),
            inner,
        );
        return;
    }

    let by_line: HashMap<u32, &crate::git::BlameLine> =
        git_blame.iter().map(|b| (b.line_no, b)).collect();

    let dim = Style::default().fg(theme.dim);
    let max_rows = inner.height as usize;
    let rows: Vec<Line> = (start..=end)
        .take(max_rows)
        .map(|disp| {
            let phys = app.display_to_physical(disp);
            let lineno = phys + 1;
            let content = app.line_text(phys).unwrap_or("");
            if let Some(ref plines) = plugin_lines {
                // Plugin blame: pre-formatted display strings, render directly.
                let prefix = plines.get(phys).map(|s| s.as_str()).unwrap_or("");
                Line::from(vec![
                    Span::styled(prefix.to_string(), dim),
                    Span::styled(format!("{lineno:>5} "), dim),
                    Span::styled(content.to_string(), Style::default().fg(theme.text)),
                ])
            } else {
                match by_line.get(&(lineno as u32)) {
                    Some(b) => Line::from(vec![
                        Span::styled(
                            format!("{} ", b.short_hash),
                            Style::default().fg(theme.accent_alt),
                        ),
                        Span::styled(
                            format!("{:<12} ", truncate(&b.author, 12)),
                            Style::default().fg(theme.accent),
                        ),
                        Span::styled(format!("{:<13} ", truncate(&b.date_relative, 13)), dim),
                        Span::styled(format!("{lineno:>5} "), dim),
                        Span::styled(content.to_string(), Style::default().fg(theme.text)),
                    ]),
                    None => Line::from(vec![
                        Span::styled(format!("{:<27} ", "(uncommitted)"), dim),
                        Span::styled(format!("{lineno:>5} "), dim),
                        Span::styled(content.to_string(), Style::default().fg(theme.text)),
                    ]),
                }
            }
        })
        .collect();

    f.render_widget(Paragraph::new(rows), inner);
}

/// Truncates `s` to at most `max` characters (by Unicode scalar value).
fn truncate(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
