//! The bug report popup.
//!
//! Renders a centered dialog containing a multiline text editor for the bug report body
//! and a read-only scrollable preview of the diagnostic report below it.
//! The user types the report body, submits with Ctrl+S / Ctrl+Enter, and cancels with Esc.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

pub(crate) fn draw_bug_report(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(state) = app.bug_report.as_mut() else {
        return;
    };
    let theme = &app.theme;

    // Centered modal: larger size to fit input + preview
    let popup = centered_rect(75, 75, area);
    app.bug_report_area = popup;
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Submit Bug Report ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    if inner.height < 15 || inner.width < 10 {
        return;
    }

    // Split inner layout
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Header instructions
            Constraint::Length(1), // Separator
            Constraint::Length(7), // Description input block
            Constraint::Length(1), // Separator
            Constraint::Min(0),    // Diagnostics preview block
            Constraint::Length(1), // Separator
            Constraint::Length(1), // Footer info
        ])
        .split(inner);

    // Header info
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            "Please describe the bug details below. Provide steps to reproduce if possible.",
            Style::default().fg(theme.text),
        )])),
        parts[0],
    );

    // Separator 1
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    // Multiline edit block
    let desc_block = Block::default()
        .title(" Description (What happened / steps) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.dim));
    let desc_inner = desc_block.inner(parts[2]);
    f.render_widget(desc_block, parts[2]);

    let edit_width = (desc_inner.width as usize).max(1);
    state.clamp_scroll(desc_inner.height as usize, edit_width);

    // Build all visual lines, wrapping long logical lines at edit_width chars
    let mut desc_lines: Vec<Line> = Vec::new();
    for (li, line_text) in state.text.iter().enumerate() {
        let char_count = line_text.chars().count();
        if char_count == 0 {
            if li == state.cursor_row && state.cursor_col == 0 {
                desc_lines.push(Line::from(Span::styled(
                    "█",
                    Style::default().fg(theme.accent_alt),
                )));
            } else {
                desc_lines.push(Line::from(""));
            }
            continue;
        }
        let num_chunks = char_count.div_ceil(edit_width);
        for chunk_idx in 0..num_chunks {
            let start = chunk_idx * edit_width;
            let end = std::cmp::min(start + edit_width, char_count);
            let chunk: String = line_text.chars().skip(start).take(end - start).collect();
            let chunk_char_count = end - start;

            if li == state.cursor_row
                && state.cursor_col >= start
                && (state.cursor_col < end || chunk_idx == num_chunks - 1)
            {
                let col_in_chunk =
                    std::cmp::min(state.cursor_col.saturating_sub(start), chunk_char_count);
                let before: String = chunk.chars().take(col_in_chunk).collect();
                let after: String = chunk.chars().skip(col_in_chunk).collect();
                desc_lines.push(Line::from(vec![
                    Span::raw(before),
                    Span::styled("█", Style::default().fg(theme.accent_alt)),
                    Span::raw(after),
                ]));
            } else {
                desc_lines.push(Line::from(vec![Span::raw(chunk)]));
            }
        }
    }

    let start_desc = state.scroll_top.min(desc_lines.len().saturating_sub(1));
    let end_desc = std::cmp::min(start_desc + desc_inner.height as usize, desc_lines.len());

    let mut visible_lines: Vec<Line> = desc_lines[start_desc..end_desc].to_vec();
    while visible_lines.len() < desc_inner.height as usize {
        visible_lines.push(Line::from(""));
    }
    f.render_widget(
        Paragraph::new(visible_lines)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(theme.text).bg(theme.background)),
        desc_inner,
    );

    // Separator 2
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[3],
    );

    // Diagnostics Preview block
    let preview_block = Block::default()
        .title(" Diagnostic Payload Preview (Read-Only) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.dim));
    let preview_inner = preview_block.inner(parts[4]);
    app.bug_report_preview_area = preview_inner;
    f.render_widget(preview_block, parts[4]);

    let body_text = state.text.join("\n");
    let report_md = if body_text.trim().is_empty() {
        state.diagnostics_markdown.clone()
    } else {
        format!(
            "## bug report body\n\n{}\n\n{}",
            body_text, state.diagnostics_markdown
        )
    };
    let preview_lines_all: Vec<String> = report_md.lines().map(String::from).collect();
    let total_rows = preview_lines_all.len();
    let visible = preview_inner.height as usize;
    let max_scroll = total_rows.saturating_sub(visible);

    state.preview_scroll.clamp(max_scroll);

    let start_prev = state.preview_scroll.scroll;
    let end_prev = (start_prev + visible).min(total_rows);

    let mut preview_lines = Vec::new();
    for line in &preview_lines_all[start_prev..end_prev] {
        preview_lines.push(Line::from(Span::raw(line.clone())));
    }
    while preview_lines.len() < visible {
        preview_lines.push(Line::from(""));
    }

    f.render_widget(
        Paragraph::new(preview_lines).style(Style::default().fg(theme.text).bg(theme.background)),
        preview_inner,
    );

    // Draw scroll indicator for preview if it overflows
    if max_scroll > 0 {
        let indicator_y = if total_rows > 0 {
            (state.preview_scroll.scroll as f64 * preview_inner.height.saturating_sub(2) as f64
                / max_scroll as f64)
                .round() as u16
        } else {
            0
        };
        let indicator_y = indicator_y
            .saturating_add(preview_inner.y)
            .min(preview_inner.bottom().saturating_sub(2));
        let indicator_chars = if state.preview_scroll.scroll == 0 {
            " ▲ "
        } else if state.preview_scroll.scroll >= max_scroll {
            " ▼ "
        } else {
            " ║ "
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                indicator_chars,
                Style::default().fg(theme.dim),
            ))),
            Rect {
                x: preview_inner.right().saturating_sub(3),
                y: indicator_y,
                width: 3,
                height: 1,
            },
        );
    }

    // Separator 3
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[5],
    );

    // Footer info
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Ctrl+S / Ctrl+Enter: ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Submit & Save  ", Style::default().fg(theme.dim)),
            Span::styled(
                "Esc: ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Cancel  ", Style::default().fg(theme.dim)),
            Span::styled(
                "PgUp / PgDown / Mouse Wheel: ",
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Scroll Preview", Style::default().fg(theme.dim)),
        ])),
        parts[6],
    );
}

#[cfg(test)]
#[path = "bug_report_test.rs"]
mod tests;
