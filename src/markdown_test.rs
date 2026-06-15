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
    let result = render("| a | b |\n|---|---|\n| 1 | 2 |", &default_theme());
    assert!(
        result[0][0].1.contains('┌'),
        "top border: {:?}",
        result[0][0].1
    );
    assert!(result.last().unwrap().is_empty(), "trailing blank");
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
