use ratatui::style::Color;
use serde::Deserialize;

/// The active color palette. Field names are semantic roles, not literal
/// colors, so a theme can remap the whole UI. `Default` reproduces the
/// original hardcoded look.
#[derive(Clone)]
pub struct Theme {
    pub accent: Color,       // focused borders, primary highlights
    pub accent_alt: Color,   // popup chrome, keys, prompts
    pub dim: Color,          // unfocused borders, gutters, hints, rules
    pub text: Color,         // emphasized/default text
    pub dir: Color,          // directory entries in the tree
    pub file: Color,         // file entries in the tree
    pub selection_bg: Color, // selected row / status bar background
    pub selection_fg: Color, // selected row foreground in popups
    pub heading1: Color,     // markdown H1 / table headers
    pub heading2: Color,     // markdown H2
    pub heading3: Color,     // markdown H3
    pub code: Color,         // inline code / code blocks
    pub diff_add: Color,     // added lines in a diff
    pub diff_del: Color,     // removed lines in a diff
    pub syntax: String,      // syntect theme name for file contents
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            accent: Color::Cyan,
            accent_alt: Color::Yellow,
            dim: Color::DarkGray,
            text: Color::White,
            dir: Color::Blue,
            file: Color::Reset,
            selection_bg: Color::DarkGray,
            selection_fg: Color::Yellow,
            heading1: Color::LightCyan,
            heading2: Color::LightYellow,
            heading3: Color::LightGreen,
            code: Color::LightYellow,
            diff_add: Color::Green,
            diff_del: Color::Red,
            syntax: "base16-ocean.dark".to_string(),
        }
    }
}

/// `[theme]` overrides from tv.toml. Any field left unset keeps the default.
/// Colors accept names ("cyan", "lightyellow", "reset") or hex ("#aabbcc");
/// `syntax` is a syntect theme name.
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct ThemeConfig {
    accent: Option<String>,
    accent_alt: Option<String>,
    dim: Option<String>,
    text: Option<String>,
    dir: Option<String>,
    file: Option<String>,
    selection_bg: Option<String>,
    selection_fg: Option<String>,
    heading1: Option<String>,
    heading2: Option<String>,
    heading3: Option<String>,
    code: Option<String>,
    diff_add: Option<String>,
    diff_del: Option<String>,
    syntax: Option<String>,
}

impl ThemeConfig {
    /// Builds a runtime `Theme`, parsing each override and falling back to the
    /// default for anything unset or invalid.
    pub fn resolve(&self) -> Theme {
        let d = Theme::default();
        let col =
            |o: &Option<String>, def: Color| o.as_deref().and_then(parse_color).unwrap_or(def);
        Theme {
            accent: col(&self.accent, d.accent),
            accent_alt: col(&self.accent_alt, d.accent_alt),
            dim: col(&self.dim, d.dim),
            text: col(&self.text, d.text),
            dir: col(&self.dir, d.dir),
            file: col(&self.file, d.file),
            selection_bg: col(&self.selection_bg, d.selection_bg),
            selection_fg: col(&self.selection_fg, d.selection_fg),
            heading1: col(&self.heading1, d.heading1),
            heading2: col(&self.heading2, d.heading2),
            heading3: col(&self.heading3, d.heading3),
            code: col(&self.code, d.code),
            diff_add: col(&self.diff_add, d.diff_add),
            diff_del: col(&self.diff_del, d.diff_del),
            syntax: self.syntax.clone().unwrap_or(d.syntax),
        }
    }
}

/// Parses a color name or `#rrggbb` hex string into a ratatui `Color`.
pub fn parse_color(s: &str) -> Option<Color> {
    let t = s.trim().to_ascii_lowercase();
    let c = match t.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ if t.starts_with('#') => return parse_hex(&t),
        _ => return None,
    };
    Some(c)
}

fn parse_hex(s: &str) -> Option<Color> {
    if h.len() != 6 || !h.is_ascii() {
    if h.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_and_hex() {
        assert_eq!(parse_color("cyan"), Some(Color::Cyan));
        assert_eq!(parse_color("LightYellow"), Some(Color::LightYellow));
        assert_eq!(parse_color(" reset "), Some(Color::Reset));
        assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
        assert_eq!(parse_color("nonsense"), None);
        assert_eq!(parse_color("#fff"), None); // must be 6 digits
    }

    #[test]
    fn default_is_used_for_unset_and_invalid() {
        let cfg = ThemeConfig {
            accent: Some("#010203".into()),
            dim: Some("not-a-color".into()),
            ..Default::default()
        };
        let t = cfg.resolve();
        assert_eq!(t.accent, Color::Rgb(1, 2, 3));
        assert_eq!(t.dim, Theme::default().dim); // invalid -> default
        assert_eq!(t.diff_add, Theme::default().diff_add); // unset -> default
        assert_eq!(t.syntax, "base16-ocean.dark");
    }
}
