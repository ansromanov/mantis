use super::*;

#[test]
fn render_simple_paragraph() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("Hello world", &theme);
    assert!(!result.is_empty());
    assert!(result.iter().any(|l| l.contains("Hello world")));
}

#[test]
fn render_heading() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("# Title", &theme);
    let heading_line = result.iter().find(|l| l.contains("Title")).unwrap();
    assert!(heading_line.contains("\x1b[1;"), "heading should be bold");
    assert!(heading_line.contains("Title"));
}

#[test]
fn render_bold_and_italic() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("**bold** and *italic*", &theme);
    let line = result.iter().find(|l| l.contains("bold")).unwrap();
    assert!(line.contains("\x1b[1m"), "bold uses ANSI code 1");
    let italic_line = result.iter().find(|l| l.contains("italic")).unwrap();
    assert!(italic_line.contains("\x1b[3m"), "italic uses ANSI code 3");
}

#[test]
fn render_code_block() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("```\nlet x = 1;\n```", &theme);
    let code_lines: Vec<&String> = result.iter().filter(|l| l.contains("let x = 1;")).collect();
    assert!(
        !code_lines.is_empty(),
        "code block content should be rendered"
    );
    assert!(
        code_lines[0].contains("\x1b[0m"),
        "code content should have reset ANSI"
    );
}

#[test]
fn render_unordered_list() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("- item\n- another", &theme);
    assert!(result.iter().any(|l| l.contains("item")));
    assert!(result.iter().any(|l| l.contains("another")));
    assert!(
        result.iter().any(|l| l.contains("•")),
        "list items should use bullet"
    );
}

#[test]
fn render_blockquote() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("> quoted text", &theme);
    let qline = result.iter().find(|l| l.contains("quoted text")).unwrap();
    assert!(qline.contains("│"), "blockquote should have vertical bar");
}

#[test]
fn render_horizontal_rule() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("---", &theme);
    assert!(result.iter().any(|l| l.contains("─") && l.len() > 10));
}

#[test]
fn render_inline_code() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("use `map()` here", &theme);
    let line = result.iter().find(|l| l.contains("map")).unwrap();
    assert!(line.contains("`map()`"));
}

#[test]
fn render_strikethrough() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("~~struck~~", &theme);
    let line = result.iter().find(|l| l.contains("struck")).unwrap();
    assert!(line.contains("\x1b[9m"), "strikethrough uses ANSI code 9");
}

#[test]
fn render_link() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("[text](url)", &theme);
    assert!(result.iter().any(|l| l.contains("text")));
}

#[test]
fn render_task_list_checked() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("- [x] done", &theme);
    assert!(result.iter().any(|l| l.contains("done")));
    assert!(
        result.iter().any(|l| l.contains("☑")),
        "checked task should show ☑"
    );
}

#[test]
fn render_task_list_unchecked() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("- [ ] todo", &theme);
    assert!(result.iter().any(|l| l.contains("todo")));
    assert!(
        result.iter().any(|l| l.contains("☐")),
        "unchecked task should show ☐"
    );
}

#[test]
fn render_empty_string() {
    let theme = MarkdownColors::default_theme();
    let result = render_to_ansi("", &theme);
    assert!(result.is_empty() || result == vec![String::new()]);
}

#[test]
fn render_table() {
    let theme = MarkdownColors::default_theme();
    let md = "| a | b |\n|---|---|\n| 1 | 2 |";
    let result = render_to_ansi(md, &theme);
    // Should have border characters and cell content
    let text = result.join("\n");
    assert!(
        text.contains('a'),
        "table should contain cell 'a': {text:?}"
    );
    assert!(text.contains('b'), "table should contain cell 'b'");
    assert!(text.contains('1'), "table should contain cell '1'");
    assert!(text.contains('2'), "table should contain cell '2'");
    assert!(
        result.iter().any(|l| l.contains('┌')),
        "table should have top border"
    );
    assert!(
        result.iter().any(|l| l.contains('└')),
        "table should have bottom border"
    );
}

#[test]
fn hex_to_truecolor_converts_valid_hex() {
    assert_eq!(
        hex_to_truecolor("#0550ae"),
        Some("38;2;5;80;174".to_string())
    );
}

#[test]
fn hex_to_truecolor_rejects_malformed_hex() {
    assert_eq!(hex_to_truecolor("0550ae"), None, "missing leading #");
    assert_eq!(hex_to_truecolor("#fff"), None, "wrong length");
    assert_eq!(hex_to_truecolor("#zzzzzz"), None, "non-hex digits");
}

#[test]
fn render_heading_levels() {
    let theme = MarkdownColors::default_theme();
    for md in ["# H1", "## H2", "### H3"] {
        let result = render_to_ansi(md, &theme);
        let text = &md[md.find('#').unwrap_or(0)..];
        let word = text.trim_start_matches('#').trim();
        assert!(
            result.iter().any(|l| l.contains(word)),
            "should contain {word}"
        );
    }
}

#[test]
fn visible_width_unicode() {
    assert_eq!(visible_width("a"), 1);
    assert_eq!(visible_width("你好"), 4, "CJK chars are 2 columns each");
}

#[test]
fn from_theme_uses_the_theme_roles_as_truecolor() {
    let theme = crate::theme::Theme {
        heading1: ratatui::style::Color::Rgb(5, 80, 174),
        text: ratatui::style::Color::Rgb(56, 56, 56),
        ..Default::default()
    };
    let colors = MarkdownColors::from_theme(&theme);
    assert_eq!(colors.heading1, "38;2;5;80;174");
    assert_eq!(colors.text, "38;2;56;56;56");
}
