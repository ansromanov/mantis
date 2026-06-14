use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

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
    pub git_clean: Color,    // clean working-tree indicator
    pub git_dirty: Color,    // dirty working-tree indicator
    pub syntax: String,      // syntect theme name for file contents
}

impl Default for Theme {
    fn default() -> Self {
        Theme::load("default").expect("default theme should always load")
    }
}

/// Intermediate struct for deserializing theme TOML files.
#[derive(Deserialize)]
struct ThemeToml {
    background: String,
    accent: String,
    accent_alt: String,
    dim: String,
    text: String,
    dir: String,
    file: String,
    selection_bg: String,
    selection_fg: String,
    heading1: String,
    heading2: String,
    heading3: String,
    code: String,
    diff_add: String,
    diff_del: String,
    git_clean: String,
    git_dirty: String,
    syntax: String,
}

impl Theme {
    /// Build a `Theme` from a TOML string. Returns `None` if any field is
    /// invalid.
    fn from_toml(toml_str: &str) -> Option<Theme> {
        let tf: ThemeToml = toml::from_str(toml_str).ok()?;
        Some(Theme {
            background: parse_color(&tf.background)?,
            accent: parse_color(&tf.accent)?,
            accent_alt: parse_color(&tf.accent_alt)?,
            dim: parse_color(&tf.dim)?,
            text: parse_color(&tf.text)?,
            dir: parse_color(&tf.dir)?,
            file: parse_color(&tf.file)?,
            selection_bg: parse_color(&tf.selection_bg)?,
            selection_fg: parse_color(&tf.selection_fg)?,
            heading1: parse_color(&tf.heading1)?,
            heading2: parse_color(&tf.heading2)?,
            heading3: parse_color(&tf.heading3)?,
            code: parse_color(&tf.code)?,
            diff_add: parse_color(&tf.diff_add)?,
            diff_del: parse_color(&tf.diff_del)?,
            git_clean: parse_color(&tf.git_clean)?,
            git_dirty: parse_color(&tf.git_dirty)?,
            syntax: tf.syntax,
        })
    }

    /// Load a theme by name. Tries the user themes directory first, then
    /// falls back to the bundled embedded themes.
    pub fn load(name: &str) -> Option<Theme> {
        // Try user themes directory first.
        if let Some(dir) = user_themes_dir() {
            let path = dir.join(format!("{name}.toml"));
            if let Ok(s) = fs::read_to_string(&path) {
                if let Some(theme) = Theme::from_toml(&s) {
                    return Some(theme);
                }
            }
        }
        // Fall back to embedded themes.
        all_embedded().get(name).cloned()
    }

    /// Discover all available themes. Returns a list of (name, theme) pairs
    /// preserving the built-in order, with user themes overriding or extending.
    pub fn discover_all() -> Vec<(String, Theme)> {
        let mut themes: Vec<(String, Theme)> = EMBEDDED_MANIFEST
            .iter()
            .filter_map(|(name, _toml)| {
                all_embedded()
                    .get(name)
                    .map(|t| (name.to_string(), t.clone()))
            })
            .collect();

        if let Some(dir) = user_themes_dir() {
            if dir.is_dir() {
                let mut entries: Vec<_> = fs::read_dir(dir)
                    .into_iter()
                    .flatten()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "toml"))
                    .collect();
                entries.sort_by_key(|e| e.file_name());

                for entry in entries {
                    let path = entry.path();
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string());
                    let Some(name) = name else { continue };
                    let Ok(s) = fs::read_to_string(&path) else {
                        continue;
                    };
                    let Some(theme) = Theme::from_toml(&s) else {
                        continue;
                    };

                    if let Some(pos) = themes.iter().position(|(n, _)| n == &name) {
                        themes[pos] = (name, theme);
                    } else {
                        themes.push((name, theme));
                    }
                }
            }
        }

        themes
    }
}

// ---------------------------------------------------------------------------
// Embedded default themes
// ---------------------------------------------------------------------------

/// List of (name, toml_content) for each shipped theme.
const EMBEDDED_MANIFEST: &[(&str, &str)] = &[
    ("default", include_str!("../themes/default.toml")),
    ("monokai", include_str!("../themes/monokai.toml")),
    ("solarized", include_str!("../themes/solarized.toml")),
    ("catppuccin", include_str!("../themes/catppuccin.toml")),
    ("synthwave84", include_str!("../themes/synthwave84.toml")),
];

fn all_embedded() -> &'static HashMap<&'static str, Theme> {
    static ALL_EMBEDDED: OnceLock<HashMap<&'static str, Theme>> = OnceLock::new();
    ALL_EMBEDDED.get_or_init(|| {
        let mut m = HashMap::new();
        for (name, toml) in EMBEDDED_MANIFEST {
            if let Some(theme) = Theme::from_toml(toml) {
                m.insert(*name, theme);
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------
// User themes directory
// ---------------------------------------------------------------------------

/// Returns the path to `~/.config/tree-viewer/themes/`.
fn user_themes_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("tree-viewer").join("themes"))
}

/// Copies every bundled theme to the user themes directory if it doesn't
/// already exist there, so users have local files to reference or edit.
pub fn install_embedded_themes() {
    let Some(dir) = user_themes_dir() else {
        return;
    };
    let _ = fs::create_dir_all(&dir);
    for (name, toml) in EMBEDDED_MANIFEST {
        let path = dir.join(format!("{name}.toml"));
        if !path.exists() {
            let _ = fs::write(&path, toml);
        }
    }
}

// ---------------------------------------------------------------------------
// ThemeConfig – user overrides from tv.toml
// ---------------------------------------------------------------------------

/// `[theme]` overrides from tv.toml. `name` selects a theme from the
/// discovered set (bundled or user-installed); any other field overrides
/// that base. Unset fields keep the base value.
/// Colors accept names ("cyan", "lightyellow", "reset") or hex ("#aabbcc");
/// `syntax` is a syntect theme name.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: Option<String>,
    /// When `true`, overrides the preset's background with `Color::Reset` so
    /// the terminal's own background shows through.
    pub transparent_background: Option<bool>,
    pub accent: Option<String>,
    pub accent_alt: Option<String>,
    pub dim: Option<String>,
    pub text: Option<String>,
    pub dir: Option<String>,
    pub file: Option<String>,
    pub selection_bg: Option<String>,
    pub selection_fg: Option<String>,
    pub heading1: Option<String>,
    pub heading2: Option<String>,
    pub heading3: Option<String>,
    pub code: Option<String>,
    pub diff_add: Option<String>,
    pub diff_del: Option<String>,
    pub git_clean: Option<String>,
    pub git_dirty: Option<String>,
    pub syntax: Option<String>,
}

impl ThemeConfig {
    /// Creates a `ThemeConfig` that selects a named theme with no overrides.
    pub fn from_preset(name: &str) -> Self {
        ThemeConfig {
            name: Some(name.to_string()),
            ..Default::default()
        }
    }

    /// Builds a runtime `Theme`: starts from the named theme (or the default),
    /// then applies any per-role overrides. Unknown/invalid values are ignored.
    pub fn resolve(&self) -> Theme {
        let d = self
            .name
            .as_deref()
            .and_then(Theme::load)
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
            git_clean: col(&self.git_clean, d.git_clean),
            git_dirty: col(&self.git_dirty, d.git_dirty),
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
    fn default_theme_loads_from_embedded() {
        let t = Theme::load("default").expect("default theme must load");
        assert_eq!(t.background, Color::Reset);
        assert_eq!(t.accent, Color::Cyan);
    }

    #[test]
    fn unknown_name_returns_none() {
        assert!(Theme::load("nonexistent-theme").is_none());
    }

    #[test]
    fn all_embedded_themes_are_valid() {
        let themes = Theme::discover_all();
        assert!(themes.len() >= 5, "should have at least 5 built-in themes");
        for (name, _) in &themes {
            assert!(!name.is_empty(), "each theme must have a non-empty name");
        }
    }

    #[test]
    fn named_preset_is_the_base_and_overrides_layer_on_top() {
        let cfg = ThemeConfig {
            name: Some("monokai".into()),
            accent: Some("#000000".into()),
            ..Default::default()
        };
        let t = cfg.resolve();
        let monokai = Theme::load("monokai").unwrap();
        assert_eq!(t.accent, Color::Rgb(0, 0, 0));
        assert_eq!(t.diff_del, monokai.diff_del);
        assert_eq!(t.syntax, monokai.syntax);
    }

    #[test]
    fn background_defaults_transparent_but_presets_set_it() {
        assert_eq!(Theme::load("default").unwrap().background, Color::Reset);
        assert_eq!(
            Theme::load("monokai").unwrap().background,
            Color::Rgb(0x27, 0x28, 0x22)
        );
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
        assert_eq!(t.dim, Theme::default().dim);
        assert_eq!(t.diff_add, Theme::default().diff_add);
    }

    #[test]
    fn discover_all_includes_synthwave84() {
        let themes = Theme::discover_all();
        assert!(
            themes.iter().any(|(n, _)| n == "synthwave84"),
            "synthwave84 must be in discovered themes"
        );
    }
}
