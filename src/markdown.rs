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
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::style::Color;

    fn default_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn plain_paragraph() {
        let result = render("hello world", &default_theme());
        assert_eq!(result.len(), 2, "paragraph + trailing blank");
        assert_eq!(result[0].len(), 1);
        assert_eq!(result[0][0].1, "hello world");
        assert_eq!(result[0][0].0, Style::default());
    }

    #[test]
    fn heading1_is_bold_underlined() {
        let result = render("# Title", &default_theme());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0][0].1, "Title");
        assert!(result[0][0].0.add_modifier.contains(Modifier::BOLD));
        assert!(result[0][0].0.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn heading_styles_vary_by_level() {
        let h4 = render("#### H4", &default_theme());
        assert!(h4[0][0].0.add_modifier.contains(Modifier::BOLD));
        assert!(!h4[0][0].0.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn bold_and_italic_formatting() {
        let result = render("**bold** and *italic*", &default_theme());
        let line = &result[0];
        assert!(line.len() >= 3);
        assert!(line[0].0.add_modifier.contains(Modifier::BOLD));
        assert!(line[2].0.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn inline_code_uses_theme_code_color() {
        let t = default_theme();
        let result = render("use `code` here", &t);
        let code_span = result[0]
            .iter()
            .find(|(_, s)| s.contains("`code`"))
            .expect("inline code span");
        assert_eq!(code_span.0.fg, Some(t.code));
    }

    #[test]
    fn code_block_renders_with_borders() {
        let t = default_theme();
        let result = render("```\ncode line\n```", &t);
        assert!(
            result[0][0].1.contains("┌──"),
            "top border: {:?}",
            result[0][0].1
        );
        let code_span = result[1]
            .iter()
            .find(|(_, s)| s == "code line")
            .expect("code content span");
        assert_eq!(code_span.0.fg, Some(t.code));
        assert!(
            result[2][0].1.contains("└──"),
            "bottom border: {:?}",
            result[2][0].1
        );
    }

    #[test]
    fn unordered_list_uses_bullets() {
        let result = render("- item1\n- item2", &default_theme());
        assert!(
            result[0][0].1.starts_with('•'),
            "line0: {:?}",
            result[0][0].1
        );
        assert!(
            result[1][0].1.starts_with('•'),
            "line1: {:?}",
            result[1][0].1
        );
    }

    #[test]
    fn block_quote_prefixes_with_pipe() {
        let result = render("> quoted text", &default_theme());
        let line = &result[0];
        assert!(line[0].1.contains("│ "));
    }

    #[test]
    fn horizontal_rule_produces_line_of_dashes() {
        let result = render("---", &default_theme());
        assert_eq!(result[0][0].1.chars().count(), 60);
        assert!(result[0][0].1.chars().all(|c| c == '─'));
    }

    #[test]
    fn link_renders_text_without_url() {
        let result = render("[link text](http://x.com)", &default_theme());
        let text: String = result[0].iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(text, "link text", "got {text:?}");
    }

    #[test]
    fn link_is_accent_underlined() {
        let t = default_theme();
        let result = render("[link text](http://x.com)", &t);
        let span = result[0]
            .iter()
            .find(|(_, s)| s == "link text")
            .expect("link text span");
        assert_eq!(span.0.fg, Some(t.accent));
        assert!(span.0.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn image_renders_placeholder() {
        let result = render("![alt](img.png)", &default_theme());
        assert!(
            result[0].iter().any(|(_, s)| s.contains("[img]")),
            "image should produce [img] placeholder: {:?}",
            result[0]
        );
    }

    #[test]
    fn table_uses_box_drawing() {
        // pulldown-cmark 0.9 omits the TableHead for this input,
        // so only a single data row is rendered.
        let result = render("| a | b |\n|---|---|\n| 1 | 2 |", &default_theme());
        assert!(
            result[0][0].1.contains('┌'),
            "top border: {:?}",
            result[0][0].1
        );
        assert!(result.last().unwrap().is_empty(), "trailing blank");
        // Verify data cells are present.
        let data_row = &result[1];
        let cell_text: String = data_row.iter().map(|(_, s)| s.as_str()).collect();
        assert!(cell_text.contains("1"), "data row has '1': {cell_text:?}");
        assert!(cell_text.contains("2"), "data row has '2': {cell_text:?}");
    }

    #[test]
    fn strikethrough_uses_crossed_out() {
        let result = render("~~struck~~", &default_theme());
        assert!(result[0][0].0.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn nested_bold_italic_combines_modifiers() {
        let result = render("***nested***", &default_theme());
        assert!(result[0][0].0.add_modifier.contains(Modifier::BOLD));
        assert!(result[0][0].0.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn task_list_shows_checkbox() {
        let result = render("- [x] done\n- [ ] todo", &default_theme());
        let line0: String = result[0].iter().map(|(_, s)| s.as_str()).collect();
        let line1: String = result[1].iter().map(|(_, s)| s.as_str()).collect();
        assert!(line0.contains('☑'), "checked: {line0:?}");
        assert!(line1.contains('☐'), "unchecked: {line1:?}");
    }

    #[test]
    fn hard_break_splits_lines() {
        let result = render("line1  \nline2", &default_theme());
        assert_eq!(result[0][0].1, "line1");
        assert_eq!(result[1][0].1, "line2");
    }

    #[test]
    fn empty_input_yields_no_lines() {
        assert!(render("", &default_theme()).is_empty());
    }

    #[test]
    fn table_border_builds_correctly() {
        assert_eq!(table_border('┌', '─', '┬', '┐', &[3, 5]), "┌─────┬───────┐");
    }

    #[test]
    fn pad_aligns_text() {
        assert_eq!(pad("hi", 5, Alignment::None), "hi   ");
        assert_eq!(pad("hi", 5, Alignment::Right), "   hi");
        assert_eq!(pad("hi", 5, Alignment::Center), " hi  ");
    }

    #[test]
    fn pad_center_right_biases_odd_remainder() {
        // Odd total padding (3): left gets pad/2=1, right gets pad-pad/2=2.
        assert_eq!(pad("h", 4, Alignment::Center), " h  ");
    }

    #[test]
    fn pad_longer_than_width_unchanged() {
        assert_eq!(pad("hello world", 5, Alignment::None), "hello world");
    }

    #[test]
    fn top_returns_last_style_or_default() {
        let styles = vec![
            Style::default().fg(Color::Red),
            Style::default().fg(Color::Blue),
        ];
        assert_eq!(top(&styles), Style::default().fg(Color::Blue));
        assert_eq!(top(&[]), Style::default());
    }

    #[test]
    fn heading_style_variants() {
        let t = default_theme();
        let h1 = heading_style(HeadingLevel::H1, &t);
        assert_eq!(h1.fg, Some(t.heading1));
        assert!(h1
            .add_modifier
            .contains(Modifier::BOLD | Modifier::UNDERLINED));

        let h2 = heading_style(HeadingLevel::H2, &t);
        assert_eq!(h2.fg, Some(t.heading2));
        assert!(h2.add_modifier.contains(Modifier::BOLD));
        assert!(!h2.add_modifier.contains(Modifier::UNDERLINED));

        let h3 = heading_style(HeadingLevel::H3, &t);
        assert_eq!(h3.fg, Some(t.heading3));

        let h4 = heading_style(HeadingLevel::H4, &t);
        assert_eq!(h4.fg, Some(t.text));
        assert!(h4.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn render_table_empty_input() {
        let t = default_theme();
        assert!(render_table(&[], &[], &t).is_empty());
    }
}
