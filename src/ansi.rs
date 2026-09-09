//! ANSI escape code parser for plugin-rendered content.
//!
//! Plugins can send `set_content` actions with lines containing ANSI escape
//! codes (e.g. `\x1b[31mred\x1b[0m`). This module converts those lines into
//! ratatui `(Style, String)` spans so the content pane can render them
//! alongside natively-highlighted text.
//!
//! Supported ANSI features:
//! - 3/4-bit foreground/background colours (30–37, 40–47, 90–97, 100–107)
//! - 8-bit (256-colour) foreground/background: `\x1b[38;5;Nm` / `\x1b[48;5;Nm`
//! - 24-bit true colour foreground/background: `\x1b[38;2;R;G;Bm` / `\x1b[48;2;R;G;Bm`
//! - Bold (1), Dim (2), Italic (3), Underline (4), Strikethrough (9)
//! - Reset (0), Reset bold (21), Reset dim (22), Reset italic (23),
//!   Reset underline (24), Reset foreground (39), Reset background (49)
//! - Conceal (8), Reverse (7)
//! - SGR sequences nested within other SGR sequences are merged.
//! - Unsupported/unknown codes are silently stripped.
//!
//! The output matches the `Vec<Vec<(Style, String)>>` shape used by the
//! highlighter and markdown renderer.

use ratatui::style::{Color, Modifier, Style};

/// Parses a single line (potentially containing ANSI escape codes) into
/// `Vec<(Style, String)>` spans, ready for the content pane.
///
/// The input string may contain ANSI SGR (Select Graphic Rendition) sequences
/// of the form `\x1b[<params>m`. Non-SGR escape sequences are stripped.
///
/// When the input has no ANSI codes the result is a single span with the
/// default style for the whole string, mirroring the behaviour of the syntax
/// highlighter for unhighlighted text.
pub fn parse_ansi_line(line: &str) -> Vec<(Style, String)> {
    if line.is_empty() {
        return Vec::new();
    }
    if !line.contains('\x1b') {
        return vec![(Style::default(), line.to_string())];
    }

    let mut spans: Vec<(Style, String)> = Vec::new();
    let mut current_style = Style::default();
    let mut buf = String::new();

    let mut chars = line.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c == '\x1b' {
            let Some((_, '[')) = chars.next() else {
                continue;
            };
            let mut param = String::new();
            for (_, ch) in chars.by_ref() {
                if ch.is_ascii_alphabetic() {
                    if ch == 'm' {
                        // SGR sequence: flush any pending text then apply the style.
                        if !buf.is_empty() {
                            spans.push((current_style, std::mem::take(&mut buf)));
                        }
                        apply_sgr(&param, &mut current_style);
                    }
                    // Non-SGR CSI sequences are silently consumed without flushing.
                    break;
                }
                param.push(ch);
            }
        } else {
            buf.push(c);
        }
    }

    if !buf.is_empty() {
        spans.push((current_style, buf));
    }

    spans
}

/// Returns whether `text` contains characters or escape sequences that must be
/// sanitized before being rendered directly into the terminal.
pub fn contains_terminal_controls(text: &str) -> bool {
    text.chars().any(|c| {
        c == '\x1b'
            || ('\u{80}'..='\u{9f}').contains(&c)
            || (c.is_control() && c != '\n' && c != '\r')
            || matches!(
                c,
                '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'
                    | '\u{202b}'
                    | '\u{202c}'
                    | '\u{202d}'
                    | '\u{202e}'
                    | '\u{2066}'
                    | '\u{2067}'
                    | '\u{2068}'
                    | '\u{2069}'
            )
    })
}

/// Removes terminal control sequences and unsafe directional controls from
/// text that will be emitted through ratatui. Tabs become spaces so they cannot
/// alter the terminal grid; CSI and OSC sequences are consumed as a whole.
pub fn sanitize_terminal_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            consume_escape_sequence(&mut chars);
        } else if ('\u{80}'..='\u{9f}').contains(&c) {
            consume_c1_sequence(c, &mut chars);
        } else if c == '\t' {
            out.push(' ');
        } else if c == '\n' || c == '\r' {
            out.push(c);
        } else if c.is_control() {
            // C0 controls have no useful visual representation in a file pane.
        } else if is_directional_control(c) {
            out.push_str(&format!("<U+{:04X}>", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

fn consume_escape_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match chars.next() {
        Some('[') => consume_csi_sequence(chars),
        Some(']') => consume_osc_sequence(chars),
        Some(_) => {}
        None => {}
    }
}

fn consume_c1_sequence<I>(c: char, chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    match c {
        '\u{9b}' => consume_csi_sequence(chars),
        '\u{9d}' => consume_osc_sequence(chars),
        _ => {}
    }
}

fn consume_csi_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    for c in chars.by_ref() {
        if ('@'..='~').contains(&c) {
            break;
        }
    }
}

fn consume_osc_sequence<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    for c in chars.by_ref() {
        if c == '\u{07}' {
            break;
        }
        if c == '\x1b' {
            if chars.peek() == Some(&'\\') {
                chars.next();
            }
            break;
        }
    }
}

fn is_directional_control(c: char) -> bool {
    matches!(
        c,
        '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'
            | '\u{202b}'
            | '\u{202c}'
            | '\u{202d}'
            | '\u{202e}'
            | '\u{2066}'
            | '\u{2067}'
            | '\u{2068}'
            | '\u{2069}'
    )
}

/// Parses an SGR parameter string and applies it to `style`.
///
/// The parameter string is the part between `\x1b[` and `m`, e.g. `"1;31"`
/// for bold red. Compound colour sequences (`38;2;R;G;B`, `38;5;N`,
/// `48;2;R;G;B`, `48;5;N`) are handled as single tokens before falling
/// back to individual SGR code parsing for the remaining parameters.
fn apply_sgr(param: &str, style: &mut Style) {
    let param = param.trim();
    if param.is_empty() || param == "0" {
        *style = Style::default();
        return;
    }

    // Tokenise, keeping compound colour sequences together.
    // A compound colour starts with "38;5;", "48;5;", "38;2;", or "48;2;".
    let tokens = tokenise_sgr(param);

    for token in &tokens {
        match token.as_str() {
            "0" => *style = Style::default(),
            "1" => *style = style.add_modifier(Modifier::BOLD),
            "2" => *style = style.add_modifier(Modifier::DIM),
            "3" => *style = style.add_modifier(Modifier::ITALIC),
            "4" => *style = style.add_modifier(Modifier::UNDERLINED),
            "7" => *style = style.add_modifier(Modifier::REVERSED),
            "8" => *style = style.add_modifier(Modifier::HIDDEN),
            "9" => *style = style.add_modifier(Modifier::CROSSED_OUT),
            "21" | "22" => *style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            "23" => *style = style.remove_modifier(Modifier::ITALIC),
            "24" => *style = style.remove_modifier(Modifier::UNDERLINED),
            "27" => *style = style.remove_modifier(Modifier::REVERSED),
            "28" => *style = style.remove_modifier(Modifier::HIDDEN),
            "29" => *style = style.remove_modifier(Modifier::CROSSED_OUT),
            "39" => style.fg = None,
            "49" => style.bg = None,
            _ => {
                if let Some(rest) = token.strip_prefix("38;5;") {
                    if let Ok(n) = rest.parse::<u8>() {
                        style.fg = Some(Color::Indexed(n));
                    }
                } else if let Some(rest) = token.strip_prefix("48;5;") {
                    if let Ok(n) = rest.parse::<u8>() {
                        style.bg = Some(Color::Indexed(n));
                    }
                } else if let Some(rest) = token.strip_prefix("38;2;") {
                    if let Some(rgb) = parse_rgb(rest) {
                        style.fg = Some(rgb);
                    }
                } else if let Some(rest) = token.strip_prefix("48;2;") {
                    if let Some(rgb) = parse_rgb(rest) {
                        style.bg = Some(rgb);
                    }
                } else if let Some(n) = parse_3bit_code(simple_parse(token)) {
                    style.fg = Some(n);
                } else if let Some(n) = parse_3bit_bg_code(simple_parse(token)) {
                    style.bg = Some(n);
                }
            }
        }
    }
}

/// Splits an SGR parameter string into individual tokens, keeping compound
/// colour sequences (e.g. `38;2;R;G;B`, `38;5;N`) as single tokens.
fn tokenise_sgr(param: &str) -> Vec<String> {
    let parts: Vec<&str> = param.split(';').collect();
    let mut tokens: Vec<String> = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        if parts[i] == "38" || parts[i] == "48" {
            if i + 1 < parts.len() && parts[i + 1] == "5" && i + 2 < parts.len() {
                // 38;5;N or 48;5;N
                tokens.push(format!("{};5;{}", parts[i], parts[i + 2]));
                i += 3;
                continue;
            }
            if i + 1 < parts.len() && parts[i + 1] == "2" && i + 4 < parts.len() {
                // 38;2;R;G;B or 48;2;R;G;B
                tokens.push(format!(
                    "{};2;{};{};{}",
                    parts[i],
                    parts[i + 2],
                    parts[i + 3],
                    parts[i + 4]
                ));
                i += 5;
                continue;
            }
        }
        tokens.push(parts[i].to_string());
        i += 1;
    }
    tokens
}

/// Parses a string to `u8`, returning 0 on failure.
fn simple_parse(s: &str) -> u8 {
    s.parse().unwrap_or(0)
}

/// Returns the `Color` for a 3/4-bit foreground code (30–37, 90–97),
/// or `None` if the value is out of range.
fn parse_3bit_code(n: u8) -> Option<Color> {
    match n {
        30 => Some(Color::Black),
        31 => Some(Color::Red),
        32 => Some(Color::Green),
        33 => Some(Color::Yellow),
        34 => Some(Color::Blue),
        35 => Some(Color::Magenta),
        36 => Some(Color::Cyan),
        37 => Some(Color::White),
        90 => Some(Color::DarkGray),
        91 => Some(Color::LightRed),
        92 => Some(Color::LightGreen),
        93 => Some(Color::LightYellow),
        94 => Some(Color::LightBlue),
        95 => Some(Color::LightMagenta),
        96 => Some(Color::LightCyan),
        97 => Some(Color::White),
        _ => None,
    }
}

/// Returns the `Color` for a 3/4-bit background code (40–47, 100–107),
/// or `None` if the value is out of range.
fn parse_3bit_bg_code(n: u8) -> Option<Color> {
    match n {
        40 => Some(Color::Black),
        41 => Some(Color::Red),
        42 => Some(Color::Green),
        43 => Some(Color::Yellow),
        44 => Some(Color::Blue),
        45 => Some(Color::Magenta),
        46 => Some(Color::Cyan),
        47 => Some(Color::White),
        100 => Some(Color::DarkGray),
        101 => Some(Color::LightRed),
        102 => Some(Color::LightGreen),
        103 => Some(Color::LightYellow),
        104 => Some(Color::LightBlue),
        105 => Some(Color::LightMagenta),
        106 => Some(Color::LightCyan),
        107 => Some(Color::White),
        _ => None,
    }
}

/// Parses `R;G;B` from a 24-bit colour parameter string.
fn parse_rgb(s: &str) -> Option<Color> {
    let mut parts = s.splitn(3, ';');
    let r = parts.next()?.parse::<u8>().ok()?;
    let g = parts.next()?.parse::<u8>().ok()?;
    let b = parts.next()?.parse::<u8>().ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
#[path = "ansi_test.rs"]
mod tests;
