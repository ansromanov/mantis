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
