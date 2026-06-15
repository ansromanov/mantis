//! Markdown rendering: `pulldown-cmark` events to themed ratatui spans.
//!
//! `render` parses a markdown string and produces a `Vec` of styled lines ready
//! for the content pane. It handles headings, bordered code blocks, box-drawing
//! tables, ordered/unordered and task lists, block quotes, horizontal rules, and
//! inline formatting (bold, italic, strikethrough, inline code), with images
//! shown as a placeholder. All colors come from the active `Theme`, so rendered
//! markdown matches the rest of the UI. The output uses the same
//! `(Style, String)` span shape as the syntax highlighter, so the content
//! renderer can treat both uniformly.

use crate::theme::Theme;
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag};
use ratatui::style::{Modifier, Style};

/// Renders a markdown string into themed ratatui spans. Supports headings,
/// code blocks (bordered), tables (box-drawing), lists, block quotes,
/// horizontal rules, inline formatting (bold, italic, strikethrough, code),
/// images (placeholder), and task list markers.
pub fn render(src: &str, theme: &Theme) -> Vec<Vec<(Style, String)>> {
    let mut lines: Vec<Vec<(Style, String)>> = Vec::new();
    let mut current: Vec<(Style, String)> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut code_buf: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut list_depth: usize = 0;
    let mut bq_depth: usize = 0;

    // Table accumulation state
    let mut table_aligns: Vec<Alignment> = Vec::new();
    let mut table_rows: Vec<(bool, Vec<String>)> = Vec::new(); // (is_header, cells)
    let mut table_row_cells: Vec<String> = Vec::new();
    let mut table_cell_buf = String::new();
    let mut in_table = false;
    let mut in_table_header = false;
    let mut in_table_cell = false;

    for event in Parser::new_ext(src, Options::all()) {
        match event {
            Event::Start(Tag::Table(aligns)) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                table_aligns = aligns;
                table_rows.clear();
                in_table = true;
            }
            Event::End(Tag::Table(_)) => {
                in_table = false;
                lines.extend(render_table(&table_rows, &table_aligns, theme));
                table_rows.clear();
                table_aligns.clear();
            }
            Event::Start(Tag::TableHead) => {
                in_table_header = true;
            }
            Event::End(Tag::TableHead) => {
                in_table_header = false;
            }
            Event::Start(Tag::TableRow) => {
                table_row_cells.clear();
            }
            Event::End(Tag::TableRow) => {
                table_rows.push((in_table_header, std::mem::take(&mut table_row_cells)));
            }
            Event::Start(Tag::TableCell) => {
                table_cell_buf.clear();
                in_table_cell = true;
            }
            Event::End(Tag::TableCell) => {
                in_table_cell = false;
                table_row_cells.push(std::mem::take(&mut table_cell_buf));
            }

            Event::Start(Tag::Heading(level, _, _)) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                style_stack.push(heading_style(level, theme));
            }
            Event::End(Tag::Heading(_, _, _)) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                style_stack.pop();
                lines.push(vec![]);
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(Tag::Paragraph) => {
                if !in_table {
                    flush(&mut lines, &mut current, bq_depth, theme);
                    lines.push(vec![]);
                }
            }
            Event::Start(Tag::Strong) => {
                let s = top(&style_stack).add_modifier(Modifier::BOLD);
                style_stack.push(s);
            }
            Event::End(Tag::Strong) => {
                style_stack.pop();
            }
            Event::Start(Tag::Emphasis) => {
                let s = top(&style_stack).add_modifier(Modifier::ITALIC);
                style_stack.push(s);
            }
            Event::End(Tag::Emphasis) => {
                style_stack.pop();
            }
            Event::Start(Tag::Strikethrough) => {
                let s = top(&style_stack).add_modifier(Modifier::CROSSED_OUT);
                style_stack.push(s);
            }
            Event::End(Tag::Strikethrough) => {
                style_stack.pop();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                in_code = true;
                code_buf.clear();
            }
            Event::End(Tag::CodeBlock(_)) => {
                in_code = false;
                let dim = Style::default().fg(theme.dim);
                let code = Style::default().fg(theme.code);
                if !code_buf.is_empty() {
                    lines.push(vec![(dim, "  ┌──".to_string())]);
                    for cl in code_buf.drain(..) {
                        lines.push(vec![(dim, "  │ ".to_string()), (code, cl)]);
                    }
                    lines.push(vec![(dim, "  └──".to_string())]);
                }
                lines.push(vec![]);
            }
            Event::Start(Tag::List(_)) => {
                if list_depth == 0 {
                    flush(&mut lines, &mut current, bq_depth, theme);
                }
                list_depth += 1;
            }
            Event::End(Tag::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                if list_depth == 0 {
                    lines.push(vec![]);
                }
            }
            Event::Start(Tag::Item) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                let indent = "  ".repeat(list_depth.saturating_sub(1));
                current.push((Style::default().fg(theme.accent), format!("{}• ", indent)));
            }
            Event::End(Tag::Item) => {
                flush(&mut lines, &mut current, bq_depth, theme);
            }
            Event::Start(Tag::BlockQuote) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                bq_depth += 1;
                style_stack.push(
                    Style::default()
                        .fg(theme.dim)
                        .add_modifier(Modifier::ITALIC),
                );
            }
            Event::End(Tag::BlockQuote) => {
                flush(&mut lines, &mut current, bq_depth, theme);
                bq_depth = bq_depth.saturating_sub(1);
                style_stack.pop();
                lines.push(vec![]);
            }
            Event::Start(Tag::Link(_, _, _)) => {
                style_stack.push(
                    Style::default()
                        .fg(theme.accent)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Event::End(Tag::Link(_, _, _)) => {
                style_stack.pop();
            }
            Event::Start(Tag::Image(_, _, _)) => {
                if !in_table_cell {
                    current.push((Style::default().fg(theme.dim), "[img]".to_string()));
                }
            }
            Event::End(Tag::Image(_, _, _)) => {}
            Event::Text(t) => {
                if in_code {
                    for line in t.lines() {
                        code_buf.push(line.to_owned());
                    }
                } else if in_table_cell {
                    table_cell_buf.push_str(&t);
                } else {
                    let s = top(&style_stack);
                    current.push((s, t.to_string()));
                }
            }
            Event::Code(t) => {
                if in_table_cell {
                    table_cell_buf.push('`');
                    table_cell_buf.push_str(&t);
                    table_cell_buf.push('`');
                } else {
                    current.push((Style::default().fg(theme.code), format!("`{}`", t)));
                }
            }
            Event::SoftBreak => {
                if !in_table_cell {
                    current.push((top(&style_stack), " ".to_string()));
                }
            }
            Event::HardBreak => {
                if !in_table_cell {
                    flush(&mut lines, &mut current, bq_depth, theme);
                }
            }
            Event::Rule => {
                flush(&mut lines, &mut current, bq_depth, theme);
                lines.push(vec![(Style::default().fg(theme.dim), "─".repeat(60))]);
                lines.push(vec![]);
            }
            Event::TaskListMarker(checked) => {
                if let Some(last) = current.last_mut() {
                    let trimmed = last.1.trim_end_matches("• ").to_string();
                    last.1 = format!("{}{} ", trimmed, if checked { "☑" } else { "☐" });
                }
            }
            _ => {}
        }
    }
    flush(&mut lines, &mut current, bq_depth, theme);
    lines
}

/// Builds a box-drawing table from parsed markdown table rows. Calculates
/// column widths, applies alignment (left/center/right), and adds border
/// lines (─, │, ┌, ┬, ┐, etc.).
fn render_table(
    rows: &[(bool, Vec<String>)],
    aligns: &[Alignment],
    theme: &Theme,
) -> Vec<Vec<(Style, String)>> {
    if rows.is_empty() {
        return vec![];
    }
    let col_count = rows.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
    if col_count == 0 {
        return vec![];
    }

    let mut col_widths: Vec<usize> = vec![1; col_count];
    for (_, cells) in rows {
        for (i, cell) in cells.iter().enumerate() {
            if i < col_count {
                col_widths[i] = col_widths[i].max(cell.len());
            }
        }
    }

    let dim = Style::default().fg(theme.dim);
    let header_style = Style::default()
        .fg(theme.heading1)
        .add_modifier(Modifier::BOLD);
    let cell_style = Style::default();

    let mut out: Vec<Vec<(Style, String)>> = Vec::new();

    out.push(vec![(dim, table_border('┌', '─', '┬', '┐', &col_widths))]);

    for (is_header, cells) in rows {
        let mut spans: Vec<(Style, String)> = Vec::new();
        spans.push((dim, "│".to_string()));
        let style = if *is_header { header_style } else { cell_style };
        for (i, w) in col_widths.iter().enumerate() {
            let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let align = aligns.get(i).copied().unwrap_or(Alignment::None);
            spans.push((style, format!(" {} ", pad(text, *w, align))));
            spans.push((dim, "│".to_string()));
        }
        out.push(spans);

        if *is_header {
            out.push(vec![(dim, table_border('├', '─', '┼', '┤', &col_widths))]);
        }
    }

    out.push(vec![(dim, table_border('└', '─', '┴', '┘', &col_widths))]);
    out.push(vec![]);
    out
}

/// Builds a table border line with the given corner/fill/junction characters
/// and column widths (each width + 2 for padding).
fn table_border(left: char, fill: char, mid: char, right: char, widths: &[usize]) -> String {
    let mut s = String::from(left);
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..(*w + 2) {
            s.push(fill);
        }
        s.push(if i < widths.len() - 1 { mid } else { right });
    }
    s
}

/// Pads `text` to `width` columns using the given alignment. Used for table
/// cell content rendering.
fn pad(text: &str, width: usize, align: Alignment) -> String {
    match align {
        Alignment::Right => format!("{:>width$}", text, width = width),
        Alignment::Center => {
            let pad = width.saturating_sub(text.len());
            format!(
                "{}{}{}",
                " ".repeat(pad / 2),
                text,
                " ".repeat(pad - pad / 2)
            )
        }
        _ => format!("{:<width$}", text, width = width),
    }
}

/// Flushes the current line's accumulated spans into the output, prefixed
/// with block-quote markers (`│ `) when inside a block quote.
fn flush(
    lines: &mut Vec<Vec<(Style, String)>>,
    spans: &mut Vec<(Style, String)>,
    bq_depth: usize,
    theme: &Theme,
) {
    if spans.is_empty() {
        return;
    }
    let mut line: Vec<(Style, String)> = Vec::new();
    if bq_depth > 0 {
        line.push((Style::default().fg(theme.dim), "│ ".repeat(bq_depth)));
    }
    line.extend(std::mem::take(spans));
    lines.push(line);
}

/// Returns the top of the style stack, or default if empty.
fn top(stack: &[Style]) -> Style {
    stack.last().copied().unwrap_or_default()
}

/// Returns the style for a markdown heading level. H1 is underlined bold,
/// H2–H3 are bold with distinct colors, H4+ are bold text only.
fn heading_style(level: HeadingLevel, theme: &Theme) -> Style {
    match level {
        HeadingLevel::H1 => Style::default()
            .fg(theme.heading1)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        HeadingLevel::H2 => Style::default()
            .fg(theme.heading2)
            .add_modifier(Modifier::BOLD),
        HeadingLevel::H3 => Style::default()
            .fg(theme.heading3)
            .add_modifier(Modifier::BOLD),
        HeadingLevel::H4 | HeadingLevel::H5 | HeadingLevel::H6 => {
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
        }
    }
}

#[cfg(test)]
#[path = "markdown_test.rs"]
mod tests;
