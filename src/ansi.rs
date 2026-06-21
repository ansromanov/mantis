//! ANSI escape sequence parser for plugin-provided content.
//!
//! Converts ANSI SGR (Select Graphic Rendition) codes into ratatui `Style`
//! and `Span` tuples so that plugins can push pre-rendered coloured content
//! into the content pane via the `set_content` action without going through
//! `tv`'s own syntax-highlighting or markdown pipelines.
//!
//! The main entry point is [`parse_ansi_line`], which accepts a single line
//! of ANSI-encoded text (e.g. `"\x1b[1;36m# Heading\x1b[0m"`) and returns
//! a `Vec` of `(Style, String)` pairs suitable for ratatui rendering.
//!
//! Supported SGR features:
//! - 3/4-bit colours (30–37, 40–47, 90–97, 100–107)
//! - 8-bit / 256-colour palette (`38;5;N`, `48;5;N`)
//! - 24-bit true colour (`38;2;R;G;B`, `48;2;R;G;B`)
//! - Bold, italic, underline, reverse video
//! - SGR reset (`0`) and selective attribute removal (22, 23, 24, 27)
//!
//! Non-SGR CSI sequences (cursor movement, erase-in-display, etc.) are
//! silently stripped so they never appear as visible text in the content pane.

use ratatui::style::{Color, Modifier, Style};

/// Parses a single line of ANSI-encoded text into styled segments.
pub(crate) fn parse_ansi_line(line: &str) -> Vec<(Style, String)> {
    if line.is_empty() {
        return vec![(Style::default(), String::new())];
    }

    let mut result: Vec<(Style, String)> = Vec::new();
    let mut current_style = Style::default();
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut pos = 0;

    while pos < len {
        if bytes[pos] == 0x1b && pos + 1 < len && bytes[pos + 1] == b'[' {
            pos += 2;
            // Parameter bytes: digits, semicolons, and intermediate bytes.
            let param_start = pos;
            while pos < len && matches!(bytes[pos], b'0'..=b'9' | b';' | b' '..=b'/') {
                pos += 1;
            }
            if pos < len && bytes[pos] == b'm' {
                let params_str = std::str::from_utf8(&bytes[param_start..pos]).unwrap_or("");
                current_style = apply_sgr(params_str, current_style);
                pos += 1;
            } else if pos < len && bytes[pos] >= 0x40 && bytes[pos] <= 0x7e {
                // Non-SGR CSI sequence (cursor movement etc) - skip final byte.
                pos += 1;
            }
        } else {
            let text_start = pos;
            while pos < len {
                if bytes[pos] == 0x1b && pos + 1 < len && bytes[pos + 1] == b'[' {
                    break;
                }
                pos += 1;
            }
            if let Ok(text) = std::str::from_utf8(&bytes[text_start..pos]) {
                if !text.is_empty() {
                    match result.last_mut() {
                        Some((s, t)) if *s == current_style => t.push_str(text),
                        _ => result.push((current_style, text.to_string())),
                    }
                }
            }
        }
    }

    result
}

fn apply_sgr(params_str: &str, mut style: Style) -> Style {
    if params_str.is_empty() || params_str == "0" {
        return Style::default();
    }

    let parts: Vec<&str> = params_str.split(';').collect();
    let mut i = 0;

    while i < parts.len() {
        let code = match parts[i].parse::<u8>() {
            Ok(n) => n,
            Err(_) => {
                i += 1;
                continue;
            }
        };
        match code {
            0 => style = Style::default(),
            1 => style = style.add_modifier(Modifier::BOLD),
            3 => style = style.add_modifier(Modifier::ITALIC),
            4 => style = style.add_modifier(Modifier::UNDERLINED),
            7 => style = style.add_modifier(Modifier::REVERSED),
            22 => style = style.remove_modifier(Modifier::BOLD),
            23 => style = style.remove_modifier(Modifier::ITALIC),
            24 => style = style.remove_modifier(Modifier::UNDERLINED),
            27 => style = style.remove_modifier(Modifier::REVERSED),
            30..=37 => {
                style = style.fg(STANDARD[code as usize - 30]);
            }
            38 => {
                if let Some(s) = parse_extended_colour(&parts, &mut i) {
                    style = style.fg(s);
                }
            }
            39 => style = style.fg(Color::Reset),
            40..=47 => {
                style = style.bg(STANDARD[code as usize - 40]);
            }
            48 => {
                if let Some(s) = parse_extended_colour(&parts, &mut i) {
                    style = style.bg(s);
                }
            }
            49 => style = style.bg(Color::Reset),
            90..=97 => {
                style = style.fg(BRIGHT[code as usize - 90]);
            }
            100..=107 => {
                style = style.bg(BRIGHT[code as usize - 100]);
            }
            _ => {}
        }
        i += 1;
    }

    style
}

fn parse_extended_colour(parts: &[&str], i: &mut usize) -> Option<Color> {
    *i += 1;
    let kind = parts.get(*i)?.parse::<u8>().ok()?;
    match kind {
        5 => {
            *i += 1;
            let idx = parts.get(*i)?.parse::<u8>().ok()?;
            Some(Color::Indexed(idx))
        }
        2 => {
            *i += 1;
            let r = parts.get(*i)?.parse::<u8>().ok()?;
            *i += 1;
            let g = parts.get(*i)?.parse::<u8>().ok()?;
            *i += 1;
            let b = parts.get(*i)?.parse::<u8>().ok()?;
            Some(Color::Rgb(r, g, b))
        }
        _ => None,
    }
}

const STANDARD: [Color; 8] = [
    Color::Black,
    Color::Red,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Cyan,
    Color::White,
];

const BRIGHT: [Color; 8] = [
    Color::DarkGray,
    Color::LightRed,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
    Color::LightMagenta,
    Color::LightCyan,
    Color::White,
];

#[cfg(test)]
#[path = "ansi_test.rs"]
mod tests;
