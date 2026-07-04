use super::*;

#[test]
fn render_simple_paragraph() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("Hello world", &theme);
    assert!(!result.is_empty());
    assert!(result.iter().any(|l| l.contains("Hello world")));
}

#[test]
fn render_heading() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("# Title", &theme);
    let heading_line = result.iter().find(|l| l.contains("Title")).unwrap();
    assert!(heading_line.contains("\x1b[1;"), "heading should be bold");
    assert!(heading_line.contains("Title"));
}

#[test]
fn render_bold_and_italic() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("**bold** and *italic*", &theme);
    let line = result.iter().find(|l| l.contains("bold")).unwrap();
    assert!(line.contains("\x1b[1m"), "bold uses ANSI code 1");
    let italic_line = result.iter().find(|l| l.contains("italic")).unwrap();
    assert!(italic_line.contains("\x1b[3m"), "italic uses ANSI code 3");
}

#[test]
fn render_code_block() {
    let theme = ThemeColors::default_theme();
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
    let theme = ThemeColors::default_theme();
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
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("> quoted text", &theme);
    let qline = result.iter().find(|l| l.contains("quoted text")).unwrap();
    assert!(qline.contains("│"), "blockquote should have vertical bar");
}

#[test]
fn render_horizontal_rule() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("---", &theme);
    assert!(result.iter().any(|l| l.contains("─") && l.len() > 10));
}

#[test]
fn render_inline_code() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("use `map()` here", &theme);
    let line = result.iter().find(|l| l.contains("map")).unwrap();
    assert!(line.contains("`map()`"));
}

#[test]
fn render_strikethrough() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("~~struck~~", &theme);
    let line = result.iter().find(|l| l.contains("struck")).unwrap();
    assert!(line.contains("\x1b[9m"), "strikethrough uses ANSI code 9");
}

#[test]
fn render_link() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("[text](url)", &theme);
    assert!(result.iter().any(|l| l.contains("text")));
}

#[test]
fn render_task_list_checked() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("- [x] done", &theme);
    assert!(result.iter().any(|l| l.contains("done")));
    assert!(
        result.iter().any(|l| l.contains("☑")),
        "checked task should show ☑"
    );
}

#[test]
fn render_task_list_unchecked() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("- [ ] todo", &theme);
    assert!(result.iter().any(|l| l.contains("todo")));
    assert!(
        result.iter().any(|l| l.contains("☐")),
        "unchecked task should show ☐"
    );
}

#[test]
fn render_empty_string() {
    let theme = ThemeColors::default_theme();
    let result = render_to_ansi("", &theme);
    assert!(result.is_empty() || result == vec![String::new()]);
}

#[test]
fn render_table() {
    let theme = ThemeColors::default_theme();
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

fn colors_json(hex: &[(&str, &str)]) -> serde_json::Value {
    serde_json::Value::Object(
        hex.iter()
            .map(|(k, v)| (k.to_string(), serde_json::Value::String(v.to_string())))
            .collect(),
    )
}

const LIGHT_THEME_HEX: &[(&str, &str)] = &[
    ("heading1", "#0550ae"),
    ("heading2", "#953800"),
    ("heading3", "#116329"),
    ("accent", "#0066b8"),
    ("dim", "#cccccc"),
    ("code", "#d73a49"),
    ("text", "#383838"),
];

#[test]
fn theme_change_uses_host_supplied_hex_colors() {
    let mut state = PluginState::new();
    assert_eq!(state.theme.heading1, "38;5;81", "starts with the fallback");
    state.handle_theme_change(Some(&colors_json(LIGHT_THEME_HEX)));
    assert_eq!(state.theme.heading1, "38;2;5;80;174");
    assert_eq!(state.theme.text, "38;2;56;56;56");
}

#[test]
fn theme_change_without_colors_falls_back_to_default() {
    let mut state = PluginState::new();
    state.handle_theme_change(Some(&colors_json(LIGHT_THEME_HEX)));
    state.handle_theme_change(None);
    assert_eq!(state.theme.heading1, ThemeColors::default_theme().heading1);
    assert_eq!(state.theme.text, ThemeColors::default_theme().text);
}

#[test]
fn theme_change_with_incomplete_colors_falls_back_to_default() {
    let mut state = PluginState::new();
    let incomplete = colors_json(&[("heading1", "#0550ae")]);
    state.handle_theme_change(Some(&incomplete));
    assert_eq!(
        state.theme.heading1,
        ThemeColors::default_theme().heading1,
        "a partial colors object should not produce a half-built theme"
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
fn set_content_message_produces_valid_json() {
    let lines = vec!["line1".to_string(), "line2".to_string()];
    let mut buf: Vec<u8> = Vec::new();
    send_set_content(&lines, "/path/to/file.md", &mut buf);
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "action");
    assert_eq!(parsed["action"], "set_content");
    assert_eq!(parsed["params"]["path"], "/path/to/file.md");
    assert_eq!(parsed["params"]["lines"][0], "line1");
    assert_eq!(parsed["params"]["lines"][1], "line2");
}

#[test]
fn render_heading_levels() {
    let theme = ThemeColors::default_theme();
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
fn handle_open_skips_non_markdown() {
    let state = PluginState::new();
    let mut buf: Vec<u8> = Vec::new();
    // handle_open skips non-md files silently (no panic, no output)
    state.handle_open("/dev/null/nonexistent.txt", &mut buf);
    assert!(buf.is_empty(), "no output for non-md files");
}

#[test]
fn visible_width_unicode() {
    assert_eq!(visible_width("a"), 1);
    assert_eq!(visible_width("你好"), 4, "CJK chars are 2 columns each");
}
