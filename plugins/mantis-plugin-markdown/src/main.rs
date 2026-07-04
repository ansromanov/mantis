//! Bundled markdown renderer plugin for mantis.
//!
//! Implements the mantis plugin protocol to render `.md` files to ANSI escape
//! codes. On `on_file_open`, reads the file and sends `set_content` with
//! rendered lines. Responds to `on_theme_change` to re-render with matching
//! colours, and `on_keypress` for `M` (raw/rendered toggle).
//!
//! ## Theme colour mapping
//!
//! mantis theme colours are mapped to ANSI 256-colour codes:
//!
//! | Role      | ANSI colour |
//! |-----------|-------------|
//! | heading1  | 81 (light cyan)   |
//! | heading2  | 229 (light yellow) |
//! | heading3  | 120 (light green)  |
//! | accent    | 51 (cyan)          |
//! | dim       | 243 (dark gray)    |
//! | code      | 229 (light yellow) |
//! | text      | 15 (white)         |
//!
//! These can be overridden by sending `on_theme_change` with a different
//! theme name. The plugin maintains a small dictionary of theme presets,
//! including truecolor presets for the bundled light themes (`vscode-light`,
//! `solarized-light`, `catppuccin-latte`, `pink`); unknown theme names still
//! fall back to the dark default.

use std::io::{self, BufRead, Write};
use std::path::Path;

use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag};
use unicode_width::UnicodeWidthStr;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut state = PluginState::new();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event = msg["event"].as_str().unwrap_or("");
        match event {
            "init" => {}
            "on_file_open" => {
                if let Some(path) = msg["path"].as_str() {
                    state.handle_open(path, &mut stdout.lock());
                }
            }
            "on_theme_change" => {
                if let Some(theme) = msg["theme"].as_str() {
                    state.handle_theme_change(theme);
                }
            }
            "on_keypress" => {
                if let Some(key) = msg["key"].as_str() {
                    if key == "M" {
                        state.toggle_raw = !state.toggle_raw;
                        if let Some(ref path) = state.current_file {
                            state.handle_open(path, &mut stdout.lock());
                        }
                    }
                }
            }
            "on_quit" | "shutdown" => break,
            _ => {}
        }
    }
}

struct PluginState {
    current_file: Option<String>,
    theme: ThemeColors,
    toggle_raw: bool,
}

struct ThemeColors {
    heading1: String,
    heading2: String,
    heading3: String,
    accent: String,
    dim: String,
    code: String,
    text: String,
}

impl ThemeColors {
    fn default_theme() -> Self {
        ThemeColors {
            heading1: "38;5;81".into(),  // light cyan
            heading2: "38;5;229".into(), // light yellow
            heading3: "38;5;120".into(), // light green
            accent: "38;5;51".into(),    // cyan
            dim: "38;5;243".into(),      // dark gray
            code: "38;5;229".into(),     // light yellow
            text: "38;5;15".into(),      // white
        }
    }

    fn monokai() -> Self {
        ThemeColors {
            heading1: "38;5;81".into(),
            heading2: "38;5;222".into(),
            heading3: "38;5;119".into(),
            accent: "38;5;141".into(),
            dim: "38;5;242".into(),
            code: "38;5;222".into(),
            text: "38;5;15".into(),
        }
    }

    fn solarized() -> Self {
        ThemeColors {
            heading1: "38;5;37".into(),
            heading2: "38;5;136".into(),
            heading3: "38;5;64".into(),
            accent: "38;5;37".into(),
            dim: "38;5;240".into(),
            code: "38;5;136".into(),
            text: "38;5;12".into(),
        }
    }

    fn catppuccin() -> Self {
        ThemeColors {
            heading1: "38;5;117".into(),
            heading2: "38;5;222".into(),
            heading3: "38;5;157".into(),
            accent: "38;5;183".into(),
            dim: "38;5;242".into(),
            code: "38;5;222".into(),
            text: "38;5;15".into(),
        }
    }

    fn synthwave84() -> Self {
        ThemeColors {
            heading1: "38;5;213".into(),
            heading2: "38;5;226".into(),
            heading3: "38;5;51".into(),
            accent: "38;5;207".into(),
            dim: "38;5;243".into(),
            code: "38;5;226".into(),
            text: "38;5;15".into(),
        }
    }

    fn vscode_light() -> Self {
        ThemeColors {
            heading1: "38;2;5;80;174".into(),
            heading2: "38;2;149;56;0".into(),
            heading3: "38;2;17;99;41".into(),
            accent: "38;2;0;102;184".into(),
            dim: "38;2;140;140;140".into(),
            code: "38;2;215;58;73".into(),
            text: "38;2;56;56;56".into(),
        }
    }

    fn solarized_light() -> Self {
        ThemeColors {
            heading1: "38;2;42;161;152".into(),
            heading2: "38;2;181;137;0".into(),
            heading3: "38;2;133;153;0".into(),
            accent: "38;2;38;139;210".into(),
            dim: "38;2;131;148;150".into(),
            code: "38;2;181;137;0".into(),
            text: "38;2;101;123;131".into(),
        }
    }

    fn catppuccin_latte() -> Self {
        ThemeColors {
            heading1: "38;2;4;165;229".into(),
            heading2: "38;2;223;142;29".into(),
            heading3: "38;2;64;160;43".into(),
            accent: "38;2;30;102;245".into(),
            dim: "38;2;108;111;133".into(),
            code: "38;2;223;142;29".into(),
            text: "38;2;76;79;105".into(),
        }
    }

    fn pink() -> Self {
        ThemeColors {
            heading1: "38;2;194;24;91".into(),
            heading2: "38;2;204;93;232".into(),
            heading3: "38;2;123;31;162".into(),
            accent: "38;2;214;51;132".into(),
            dim: "38;2;168;110;130".into(),
            code: "38;2;204;93;232".into(),
            text: "38;2;74;26;44".into(),
        }
    }
}

impl PluginState {
    fn new() -> Self {
        PluginState {
            current_file: None,
            theme: ThemeColors::default_theme(),
            toggle_raw: false,
        }
    }

    fn handle_theme_change(&mut self, theme_name: &str) {
        self.theme = match theme_name {
            "monokai" => ThemeColors::monokai(),
            "solarized" => ThemeColors::solarized(),
            "catppuccin" => ThemeColors::catppuccin(),
            "synthwave84" => ThemeColors::synthwave84(),
            "vscode-light" => ThemeColors::vscode_light(),
            "solarized-light" => ThemeColors::solarized_light(),
            "catppuccin-latte" => ThemeColors::catppuccin_latte(),
            "pink" => ThemeColors::pink(),
            _ => ThemeColors::default_theme(),
        };
        // Re-render the current file if one is open.
        if let Some(ref path) = self.current_file.clone() {
            self.handle_open(path, &mut io::stdout().lock());
        }
    }

    fn handle_open(&self, path_str: &str, out: &mut impl Write) {
        let path = Path::new(path_str);
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_md = matches!(ext, "md" | "markdown");
        if !is_md {
            return;
        }
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(_) => return,
        };
        let rendered = if self.toggle_raw {
            // When toggled, send raw file content as plain text.
            let lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();
            send_set_content(&lines, path_str, out);
            return;
        } else {
            render_to_ansi(&src, &self.theme)
        };
        let ansi_lines: Vec<String> = rendered;
        send_set_content(&ansi_lines, path_str, out);
    }
}

fn ansi(code: &str, text: &str) -> String {
    format!("\x1b[{code}m{text}\x1b[0m")
}

fn render_to_ansi(src: &str, theme: &ThemeColors) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut style_stack: Vec<String> = Vec::new();
    let mut code_buf: Vec<String> = Vec::new();
    let mut in_code = false;
    let mut list_depth: usize = 0;
    let mut bq_depth: usize = 0;
    let mut in_table = false;
    let mut in_table_header = false;
    let mut in_table_cell = false;
    let mut table_aligns: Vec<Alignment> = Vec::new();
    let mut table_rows: Vec<(bool, Vec<String>)> = Vec::new();
    let mut table_row_cells: Vec<String> = Vec::new();
    let mut table_cell_buf = String::new();

    for event in Parser::new_ext(src, Options::all()) {
        match event {
            Event::Start(Tag::Table(aligns)) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                table_aligns = aligns;
                table_rows.clear();
                in_table = true;
            }
            Event::End(Tag::Table(_)) => {
                in_table = false;
                render_table_ansi(&table_rows, &table_aligns, theme, &mut lines);
                table_rows.clear();
                table_aligns.clear();
            }
            Event::Start(Tag::TableHead) => {
                in_table_header = true;
                table_row_cells.clear();
            }
            Event::End(Tag::TableHead) => {
                in_table_header = false;
                if !table_row_cells.is_empty() {
                    table_rows.push((true, std::mem::take(&mut table_row_cells)));
                }
            }
            Event::Start(Tag::TableRow) => table_row_cells.clear(),
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
                flush_line(&mut lines, &mut current, bq_depth, theme);
                style_stack.push(heading_ansi(level, theme));
            }
            Event::End(Tag::Heading(_, _, _)) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                style_stack.pop();
                lines.push(String::new());
            }
            Event::Start(Tag::Paragraph) => {}
            Event::End(Tag::Paragraph) => {
                if !in_table {
                    flush_line(&mut lines, &mut current, bq_depth, theme);
                    lines.push(String::new());
                }
            }
            Event::Start(Tag::Strong) => style_stack.push("1".to_string()),
            Event::End(Tag::Strong) => {
                style_stack.pop();
            }
            Event::Start(Tag::Emphasis) => style_stack.push("3".to_string()),
            Event::End(Tag::Emphasis) => {
                style_stack.pop();
            }
            Event::Start(Tag::Strikethrough) => style_stack.push("9".to_string()),
            Event::End(Tag::Strikethrough) => {
                style_stack.pop();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                in_code = true;
                code_buf.clear();
            }
            Event::End(Tag::CodeBlock(_)) => {
                in_code = false;
                if !code_buf.is_empty() {
                    let dim = &theme.dim;
                    let code = &theme.code;
                    lines.push(format!("\x1b[{dim}m  ┌──\x1b[0m"));
                    for cl in code_buf.drain(..) {
                        lines.push(format!("\x1b[{dim}m  │ \x1b[0m\x1b[{code}m{cl}\x1b[0m"));
                    }
                    lines.push(format!("\x1b[{dim}m  └──\x1b[0m"));
                }
                lines.push(String::new());
            }
            Event::Start(Tag::List(_)) => {
                if list_depth == 0 {
                    flush_line(&mut lines, &mut current, bq_depth, theme);
                }
                list_depth += 1;
            }
            Event::End(Tag::List(_)) => {
                list_depth = list_depth.saturating_sub(1);
                if list_depth == 0 {
                    lines.push(String::new());
                }
            }
            Event::Start(Tag::Item) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                let indent = "  ".repeat(list_depth.saturating_sub(1));
                current = format!("\x1b[{accent}m{indent}• \x1b[0m", accent = theme.accent);
            }
            Event::End(Tag::Item) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
            }
            Event::Start(Tag::BlockQuote) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                bq_depth += 1;
                style_stack.push(theme.dim.clone());
            }
            Event::End(Tag::BlockQuote) => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                bq_depth = bq_depth.saturating_sub(1);
                style_stack.pop();
                lines.push(String::new());
            }
            Event::Start(Tag::Link(_, _, _)) => {
                style_stack.push(theme.accent.clone());
            }
            Event::End(Tag::Link(_, _, _)) => {
                style_stack.pop();
            }
            Event::Start(Tag::Image(_, _, _)) => {
                if !in_table_cell {
                    current.push_str(&ansi(&theme.dim, "[img]"));
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
                    let combined = style_stack.join(";");
                    current.push_str(&apply_style(&combined, &t));
                }
            }
            Event::Code(t) => {
                if in_table_cell {
                    table_cell_buf.push('`');
                    table_cell_buf.push_str(&t);
                    table_cell_buf.push('`');
                } else {
                    current.push_str(&ansi(&theme.code, &format!("`{t}`")));
                }
            }
            Event::SoftBreak => {
                if !in_table_cell {
                    current.push(' ');
                }
            }
            Event::HardBreak => {
                if !in_table_cell {
                    flush_line(&mut lines, &mut current, bq_depth, theme);
                }
            }
            Event::Rule => {
                flush_line(&mut lines, &mut current, bq_depth, theme);
                lines.push(ansi(&theme.dim, &"─".repeat(60)));
                lines.push(String::new());
            }
            Event::TaskListMarker(checked) => {
                // TaskListMarker fires immediately after Start(Item) sets
                // `current` to the bullet prefix. Replace the bullet in
                // `current` directly — lines hasn't been flushed yet.
                let indent = "  ".repeat(list_depth.saturating_sub(1));
                let marker = if checked { "☑" } else { "☐" };
                current = format!(
                    "\x1b[{accent}m{indent}{marker} \x1b[0m",
                    accent = theme.accent
                );
            }
            _ => {}
        }
    }
    flush_line(&mut lines, &mut current, bq_depth, theme);
    lines
}

fn apply_style(style: &str, text: &str) -> String {
    if style.is_empty() || text.is_empty() {
        return text.to_string();
    }
    // style is an ANSI code like "38;5;81" or a modifier like "1"
    if style.contains(';') || (style.len() <= 2 && style.chars().all(|c| c.is_ascii_digit())) {
        ansi(style, text)
    } else {
        text.to_string()
    }
}

fn flush_line(lines: &mut Vec<String>, current: &mut String, bq_depth: usize, theme: &ThemeColors) {
    if current.is_empty() {
        return;
    }
    let mut line = String::new();
    if bq_depth > 0 {
        let bq_marker = "│ ".repeat(bq_depth);
        line.push_str(&ansi(&theme.dim, &bq_marker));
    }
    line.push_str(current);
    lines.push(line);
    current.clear();
}

fn heading_ansi(level: HeadingLevel, theme: &ThemeColors) -> String {
    match level {
        HeadingLevel::H1 => format!("1;{}", theme.heading1),
        HeadingLevel::H2 => format!("1;{}", theme.heading2),
        HeadingLevel::H3 => format!("1;{}", theme.heading3),
        HeadingLevel::H4 | HeadingLevel::H5 | HeadingLevel::H6 => {
            format!("1;{}", theme.text)
        }
    }
}

fn send_set_content(lines: &[String], path: &str, out: &mut impl Write) {
    let json_lines: Vec<serde_json::Value> = lines
        .iter()
        .map(|l| serde_json::Value::String(l.clone()))
        .collect();
    let msg = serde_json::json!({
        "event": "action",
        "action": "set_content",
        "params": {
            "lines": json_lines,
            "path": path
        }
    });
    let _ = writeln!(out, "{}", serde_json::to_string(&msg).unwrap());
    let _ = out.flush();
}

// ─── Table Rendering ───────────────────────────────────────────────────────

fn render_table_ansi(
    rows: &[(bool, Vec<String>)],
    aligns: &[Alignment],
    theme: &ThemeColors,
    out: &mut Vec<String>,
) {
    if rows.is_empty() {
        return;
    }
    let col_count = rows.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
    if col_count == 0 {
        return;
    }
    let mut col_widths: Vec<usize> = vec![1; col_count];
    for (_, cells) in rows {
        for (i, cell) in cells.iter().enumerate() {
            if i < col_count {
                col_widths[i] = col_widths[i].max(visible_width(cell));
            }
        }
    }
    let dim = &theme.dim;
    let header = format!("1;{}", theme.heading1);

    out.push(format!(
        "\x1b[{dim}m{}\x1b[0m",
        table_border('┌', '─', '┬', '┐', &col_widths)
    ));
    for (is_header, cells) in rows {
        let style = if *is_header { &header } else { "" };
        let mut line = format!("\x1b[{dim}m│\x1b[0m");
        for (i, w) in col_widths.iter().enumerate() {
            let text = cells.get(i).map(|s| s.as_str()).unwrap_or("");
            let align = aligns.get(i).copied().unwrap_or(Alignment::None);
            let padded = pad_width(text, *w, align);
            line.push_str(&format!(
                "\x1b[{style}m {padded} \x1b[0m\x1b[{dim}m│\x1b[0m"
            ));
        }
        out.push(line);
        if *is_header {
            out.push(format!(
                "\x1b[{dim}m{}\x1b[0m",
                table_border('├', '─', '┼', '┤', &col_widths)
            ));
        }
    }
    out.push(format!(
        "\x1b[{dim}m{}\x1b[0m",
        table_border('└', '─', '┴', '┘', &col_widths)
    ));
    out.push(String::new());
}

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

fn pad_width(text: &str, width: usize, align: Alignment) -> String {
    let vw = visible_width(text);
    let pad = width.saturating_sub(vw);
    match align {
        Alignment::Right => format!("{}{}", " ".repeat(pad), text),
        Alignment::Center => format!(
            "{}{}{}",
            " ".repeat(pad / 2),
            text,
            " ".repeat(pad - pad / 2)
        ),
        _ => format!("{}{}", text, " ".repeat(pad)),
    }
}

fn visible_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

#[cfg(test)]
#[path = "main_test.rs"]
mod tests;
