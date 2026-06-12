use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// The active color palette. Field names are semantic roles, not literal
/// colors, so a theme can remap the whole UI. `Default` reproduces the
/// original hardcoded look.
#[derive(Clone)]
pub struct Theme {
    pub background: Color,   // panel background (Reset = terminal default)
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
            background: Color::Reset,
            accent: Color::Cyan,
            accent_alt: Color::Yellow,
            dim: Color::DarkGray,
            text: Color::White,
            dir: Color::Blue,
            file: Color::Reset,
            selection_bg: Color::Rgb(80, 80, 80),
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

/// Names of the built-in presets, in picker display order.
pub const PRESETS: &[&str] = &[
    "default",
    "monokai",
    "solarized",
    "catppuccin",
    "synthwave84",
];

impl Theme {
    /// Returns a built-in preset by name, or `None` if unknown.
    pub fn preset(name: &str) -> Option<Theme> {
        let t = match name.trim().to_ascii_lowercase().as_str() {
            "default" => Theme::default(),
            "monokai" => Theme {
                background: hex("#272822"),
                accent: hex("#66d9ef"),
                accent_alt: hex("#e6db74"),
                dim: hex("#75715e"),
                text: hex("#f8f8f2"),
                dir: hex("#66d9ef"),
                file: hex("#f8f8f2"),
                selection_bg: hex("#49483e"),
                selection_fg: hex("#e6db74"),
                heading1: hex("#66d9ef"),
                heading2: hex("#e6db74"),
                heading3: hex("#a6e22e"),
                code: hex("#e6db74"),
                diff_add: hex("#a6e22e"),
                diff_del: hex("#f92672"),
                syntax: "base16-eighties.dark".to_string(),
            },
            "solarized" => Theme {
                background: hex("#002b36"),
                accent: hex("#268bd2"),
                accent_alt: hex("#b58900"),
                dim: hex("#586e75"),
                text: hex("#93a1a1"),
                dir: hex("#268bd2"),
                file: hex("#839496"),
                selection_bg: hex("#073642"),
                selection_fg: hex("#b58900"),
                heading1: hex("#2aa198"),
                heading2: hex("#b58900"),
                heading3: hex("#859900"),
                code: hex("#b58900"),
                diff_add: hex("#859900"),
                diff_del: hex("#dc322f"),
                syntax: "Solarized (dark)".to_string(),
            },
            "catppuccin" => Theme {
                background: hex("#1e1e2e"),
                accent: hex("#89b4fa"),
                accent_alt: hex("#f9e2af"),
                dim: hex("#6c7086"),
                text: hex("#cdd6f4"),
                dir: hex("#89b4fa"),
                file: hex("#cdd6f4"),
                selection_bg: hex("#313244"),
                selection_fg: hex("#f9e2af"),
                heading1: hex("#89dceb"),
                heading2: hex("#f9e2af"),
                heading3: hex("#a6e3a1"),
                code: hex("#f9e2af"),
                diff_add: hex("#a6e3a1"),
                diff_del: hex("#f38ba8"),
                syntax: "base16-mocha.dark".to_string(),
            },
            "synthwave84" | "synthwave" => Theme {
                background: hex("#262335"),
                accent: hex("#36f9f6"),
                accent_alt: hex("#ff7edb"),
                dim: hex("#848bbd"),
                text: hex("#f0eff1"),
                dir: hex("#36f9f6"),
                file: hex("#f0eff1"),
                selection_bg: hex("#463465"),
                selection_fg: hex("#fede5d"),
                heading1: hex("#36f9f6"),
                heading2: hex("#fede5d"),
                heading3: hex("#72f1b8"),
                code: hex("#fede5d"),
                diff_add: hex("#72f1b8"),
                diff_del: hex("#f92aad"),
                syntax: "base16-eighties.dark".to_string(),
            },
            _ => return None,
        };
        Some(t)
    }
}

/// Parses a known-good hex literal used in the preset tables above.
fn hex(s: &str) -> Color {
    parse_color(s).expect("valid preset color")
}

/// `[theme]` overrides from tv.toml. `name` selects a built-in preset as the
/// base; any other field overrides that base. Unset fields keep the base value.
/// Colors accept names ("cyan", "lightyellow", "reset") or hex ("#aabbcc");
/// `syntax` is a syntect theme name.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ThemeConfig {
    name: Option<String>,
    /// When `true`, overrides the preset's background with `Color::Reset` so
    /// the terminal's own background shows through.
    transparent_background: Option<bool>,
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
    /// Creates a `ThemeConfig` that selects a named preset with no overrides.
    pub fn from_preset(name: &str) -> Self {
        ThemeConfig {
            name: Some(name.to_string()),
            ..Default::default()
        }
    }

    /// Builds a runtime `Theme`: starts from the named preset (or the default),
    /// then applies any per-role overrides. Unknown/invalid values are ignored.
    pub fn resolve(&self) -> Theme {
        let d = self
            .name
            .as_deref()
            .and_then(Theme::preset)
            .unwrap_or_default();
        let col =
            |o: &Option<String>, def: Color| o.as_deref().and_then(parse_color).unwrap_or(def);
        let background = if self.transparent_background == Some(true) {
            Color::Reset
        } else {
            d.background
        };
        Theme {
            background,
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
    let h = &s[1..];
    if h.len() != 6 || !h.is_ascii() {
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
    fn named_preset_is_the_base_and_overrides_layer_on_top() {
        let cfg = ThemeConfig {
            name: Some("monokai".into()),
            accent: Some("#000000".into()), // override just accent
            ..Default::default()
        };
        let t = cfg.resolve();
        let monokai = Theme::preset("monokai").unwrap();
        assert_eq!(t.accent, Color::Rgb(0, 0, 0)); // overridden
        assert_eq!(t.diff_del, monokai.diff_del); // from preset
        assert_eq!(t.syntax, monokai.syntax);
    }

    #[test]
    fn background_defaults_transparent_but_presets_set_it() {
        // The default theme leaves the terminal background untouched.
        assert_eq!(Theme::default().background, Color::Reset);
        // Presets ship an opaque background.
        assert_eq!(
            Theme::preset("monokai").unwrap().background,
            Color::Rgb(0x27, 0x28, 0x22)
        );
        // ...which transparent_background = true turns back off.
        let cfg = ThemeConfig {
            name: Some("monokai".into()),
            transparent_background: Some(true),
            ..Default::default()
        };
        assert_eq!(cfg.resolve().background, Color::Reset);
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
