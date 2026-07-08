//! Color themes: semantic roles, built-in presets, and color parsing.
//!
//! `Theme` is a set of named color roles (accent, dim, text, dir, file, diff
//! add/del, selection background, and so on) plus the name of a `syntect` syntax
//! theme, so a single struct remaps the entire UI without touching call sites.
//! Built-in presets (default, monokai, solarized, catppuccin, synthwave84) live
//! in `PRESETS`; `ThemeConfig` captures the user's choice and per-role overrides
//! from `mantis.toml`, which `resolve()` merges onto a base preset. Colors are
//! parsed from names or hex strings into `ratatui::style::Color`. Switching the
//! active theme reopens the current file so highlighting picks up the new syntax
//! theme.

use std::collections::HashMap;
use std::fs;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::OnceLock;

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

/// The active color palette. Field names are semantic roles, not literal
/// colors, so a theme can remap the whole UI. `Default` reproduces the
/// original hardcoded look.
#[derive(Clone)]
pub struct Theme {
    pub background: Color,     // panel background (Reset = terminal default)
    pub accent: Color,         // focused borders, primary highlights
    pub accent_alt: Color,     // popup chrome, keys, prompts
    pub dim: Color,            // unfocused borders, gutters, hints, rules
    pub text: Color,           // emphasized/default text
    pub dir: Color,            // directory entries in the tree
    pub file: Color,           // file entries in the tree
    pub selection_bg: Color,   // selected row / status bar background
    pub selection_fg: Color,   // selected row foreground in popups
    pub heading1: Color,       // markdown H1 / table headers
    pub heading2: Color,       // markdown H2
    pub heading3: Color,       // markdown H3
    pub code: Color,           // inline code / code blocks
    pub diff_add: Color,       // added lines in a diff
    pub diff_del: Color,       // removed lines in a diff
    pub git_clean: Color,      // clean working-tree indicator
    pub git_dirty: Color,      // dirty working-tree indicator
    pub git_conflict: Color,   // conflict / detached HEAD indicator
    pub git_progress: Color,   // rebase/merge in-progress indicator
    pub breadcrumb_fg: Color,  // breadcrumb path bar foreground
    pub breadcrumb_bg: Color,  // breadcrumb path bar background
    pub active_line_bg: Color, // active line cursor highlight background
    pub syntax: String,        // syntect theme name for file contents
}

impl Default for Theme {
    fn default() -> Self {
        if no_color_active() {
            Theme::monochrome()
        } else {
            let base_name = match get_detected_background() {
                Some(ThemeMode::Light) => "vscode-light",
                _ => "default",
            };
            Theme::load(base_name).unwrap_or_else(|| {
                Theme::load("default").expect("default theme should always load")
            })
        }
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
    #[serde(default)]
    git_conflict: Option<String>,
    #[serde(default)]
    git_progress: Option<String>,
    #[serde(default)]
    breadcrumb_fg: Option<String>,
    #[serde(default)]
    breadcrumb_bg: Option<String>,
    #[serde(default)]
    active_line_bg: Option<String>,
    syntax: String,
}

impl Theme {
    /// Build a `Theme` from a TOML string. Returns `None` if any field is
    /// invalid.
    fn from_toml(toml_str: &str) -> Option<Theme> {
        let tf: ThemeToml = toml::from_str(toml_str).ok()?;
        let diff_del = parse_color(&tf.diff_del)?;
        let git_dirty = parse_color(&tf.git_dirty)?;
        let accent = parse_color(&tf.accent)?;
        let background = parse_color(&tf.background)?;
        let selection_bg = parse_color(&tf.selection_bg)?;
        // New fields fall back to sensible existing roles so older theme files
        // (missing them) still parse: conflict reuses the theme's red, and an
        // in-progress rebase/merge reuses the dirty color.
        let git_conflict = tf
            .git_conflict
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(diff_del);
        let git_progress = tf
            .git_progress
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(git_dirty);
        let breadcrumb_fg = tf
            .breadcrumb_fg
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(accent);
        let breadcrumb_bg = tf
            .breadcrumb_bg
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(background);
        let active_line_bg = tf
            .active_line_bg
            .as_deref()
            .and_then(parse_color)
            .unwrap_or(selection_bg);
        Some(Theme {
            background,
            accent,
            accent_alt: parse_color(&tf.accent_alt)?,
            dim: parse_color(&tf.dim)?,
            text: parse_color(&tf.text)?,
            dir: parse_color(&tf.dir)?,
            file: parse_color(&tf.file)?,
            selection_bg,
            selection_fg: parse_color(&tf.selection_fg)?,
            heading1: parse_color(&tf.heading1)?,
            heading2: parse_color(&tf.heading2)?,
            heading3: parse_color(&tf.heading3)?,
            code: parse_color(&tf.code)?,
            diff_add: parse_color(&tf.diff_add)?,
            diff_del,
            git_clean: parse_color(&tf.git_clean)?,
            git_dirty,
            git_conflict,
            git_progress,
            breadcrumb_fg,
            breadcrumb_bg,
            active_line_bg,
            syntax: tf.syntax,
        })
    }

    /// Returns a minimal monochrome theme with no colors.
    pub fn monochrome() -> Self {
        Theme {
            background: Color::Reset,
            accent: Color::Reset,
            accent_alt: Color::Reset,
            dim: Color::Reset,
            text: Color::Reset,
            dir: Color::Reset,
            file: Color::Reset,
            selection_bg: Color::Reset,
            selection_fg: Color::Reset,
            heading1: Color::Reset,
            heading2: Color::Reset,
            heading3: Color::Reset,
            code: Color::Reset,
            diff_add: Color::Reset,
            diff_del: Color::Reset,
            git_clean: Color::Reset,
            git_dirty: Color::Reset,
            git_conflict: Color::Reset,
            git_progress: Color::Reset,
            breadcrumb_fg: Color::Reset,
            breadcrumb_bg: Color::Reset,
            active_line_bg: Color::Reset,
            syntax: String::new(),
        }
    }

    /// Returns `true` if the theme is monochrome.
    pub fn is_monochrome(&self) -> bool {
        self.selection_bg == Color::Reset
            && self.background == Color::Reset
            && self.accent == Color::Reset
            && self.text == Color::Reset
    }

    /// Returns the selection style for the theme.
    pub fn selection_style(&self) -> ratatui::style::Style {
        if self.is_monochrome() {
            ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
        } else {
            ratatui::style::Style::default()
                .bg(self.selection_bg)
                .fg(self.selection_fg)
        }
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
    ("vscode-light", include_str!("../themes/vscode-light.toml")),
    (
        "catppuccin-latte",
        include_str!("../themes/catppuccin-latte.toml"),
    ),
    (
        "solarized-light",
        include_str!("../themes/solarized-light.toml"),
    ),
    ("pink", include_str!("../themes/pink.toml")),
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

fn user_themes_dir() -> Option<PathBuf> {
    let is_test_env = cfg!(test)
        || std::env::current_exe()
            .map(|p| {
                p.parent()
                    .and_then(|parent| parent.file_name())
                    .map(|name| name == "deps")
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if is_test_env {
        #[cfg(windows)]
        let env_key = "APPDATA";
        #[cfg(not(windows))]
        let env_key = "XDG_CONFIG_HOME";

        let env_val = std::env::var_os(env_key);
        let is_isolated = env_val
            .as_ref()
            .map(|v| v.to_string_lossy().contains("tv2_theme_test"))
            .unwrap_or(false);

        if !is_isolated {
            return None;
        }
    }

    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis").join("themes"))
    }
    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
        Some(base.join("mantis").join("themes"))
    }
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
// ThemeConfig – user overrides from mantis.toml
// ---------------------------------------------------------------------------

/// `[theme]` overrides from mantis.toml. `name` selects a theme from the
/// discovered set (bundled or user-installed); any other field overrides
/// that base. Unset fields keep the base value.
/// Colors accept names ("cyan", "lightyellow", "reset") or hex ("#aabbcc");
/// `syntax` is a syntect theme name.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: Option<String>,
    pub auto_detect: Option<bool>,
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
    pub git_conflict: Option<String>,
    pub git_progress: Option<String>,
    pub breadcrumb_fg: Option<String>,
    pub breadcrumb_bg: Option<String>,
    pub active_line_bg: Option<String>,
    pub syntax: Option<String>,
}

impl ThemeConfig {
    /// A `ThemeConfig` with every field populated, used by config validation to
    /// learn the full set of recognized `[theme]` keys. Serializing the default
    /// won't do: its fields are all `None`, which TOML omits. The explicit
    /// struct literal means adding a field forces updating this, so the
    /// validation schema can't silently drift out of sync.
    #[allow(dead_code)]
    pub(crate) fn schema() -> Self {
        ThemeConfig {
            name: Some(String::new()),
            auto_detect: Some(true),
            transparent_background: Some(false),
            accent: Some(String::new()),
            accent_alt: Some(String::new()),
            dim: Some(String::new()),
            text: Some(String::new()),
            dir: Some(String::new()),
            file: Some(String::new()),
            selection_bg: Some(String::new()),
            selection_fg: Some(String::new()),
            heading1: Some(String::new()),
            heading2: Some(String::new()),
            heading3: Some(String::new()),
            code: Some(String::new()),
            diff_add: Some(String::new()),
            diff_del: Some(String::new()),
            git_clean: Some(String::new()),
            git_dirty: Some(String::new()),
            git_conflict: Some(String::new()),
            git_progress: Some(String::new()),
            breadcrumb_fg: Some(String::new()),
            breadcrumb_bg: Some(String::new()),
            active_line_bg: Some(String::new()),
            syntax: Some(String::new()),
        }
    }

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
        if no_color_active() {
            return Theme::monochrome();
        }
        let auto_detect = self.auto_detect.unwrap_or(true);
        let base_name = if let Some(ref name) = self.name {
            name.as_str()
        } else if auto_detect {
            match get_detected_background() {
                Some(ThemeMode::Light) => "vscode-light",
                _ => "default",
            }
        } else {
            "default"
        };
        let d = self
            .name
            .as_deref()
            .and_then(Theme::load)
            .unwrap_or_else(|| {
                Theme::load(base_name).unwrap_or_else(|| {
                    Theme::load("default").expect("default theme should always load")
                })
            });
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
            git_conflict: col(&self.git_conflict, d.git_conflict),
            git_progress: col(&self.git_progress, d.git_progress),
            breadcrumb_fg: col(&self.breadcrumb_fg, d.breadcrumb_fg),
            breadcrumb_bg: col(&self.breadcrumb_bg, d.breadcrumb_bg),
            active_line_bg: col(&self.active_line_bg, d.active_line_bg),
            syntax: self.syntax.clone().unwrap_or(d.syntax),
        }
    }
}

/// Renders a ratatui `Color` back to a `#rrggbb` hex string, so hosts can
/// hand a theme's actual colors to plugins over the JSON protocol instead of
/// just a theme name. Named ANSI colors are mapped to their standard xterm
/// 16-color hex values; `Reset`/`Indexed` (not used by any role plugins read)
/// fall back to a mid-gray.
pub fn color_to_hex(c: Color) -> String {
    let (r, g, b) = match c {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (205, 0, 0),
        Color::Green => (0, 205, 0),
        Color::Yellow => (205, 205, 0),
        Color::Blue => (0, 0, 238),
        Color::Magenta => (205, 0, 205),
        Color::Cyan => (0, 205, 205),
        Color::Gray => (229, 229, 229),
        Color::DarkGray => (127, 127, 127),
        Color::LightRed => (255, 0, 0),
        Color::LightGreen => (0, 255, 0),
        Color::LightYellow => (255, 255, 0),
        Color::LightBlue => (92, 92, 255),
        Color::LightMagenta => (255, 0, 255),
        Color::LightCyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::Reset | Color::Indexed(_) => (127, 127, 127),
    };
    format!("#{r:02x}{g:02x}{b:02x}")
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

static DETECTED_BACKGROUND: OnceLock<ThemeMode> = OnceLock::new();

pub fn get_detected_background() -> Option<ThemeMode> {
    DETECTED_BACKGROUND.get().copied()
}

pub fn detect_terminal_background() {
    let mode = get_colorfgbg_background()
        .or_else(query_osc_11)
        .unwrap_or(ThemeMode::Dark);
    let _ = DETECTED_BACKGROUND.set(mode);
}

fn get_colorfgbg_background() -> Option<ThemeMode> {
    if let Ok(var) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = var.split(';').collect();
        if parts.len() >= 2 {
            if let Ok(bg_idx) = parts[1].parse::<u32>() {
                if bg_idx == 7 || (9..=15).contains(&bg_idx) || bg_idx >= 244 {
                    return Some(ThemeMode::Light);
                } else {
                    return Some(ThemeMode::Dark);
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn query_osc_11() -> Option<ThemeMode> {
    use std::io::Write;
    use std::time::{Duration, Instant};

    // Stdin/stdout must be terminals
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return None;
    }

    // Write OSC 11 query
    let mut stdout = std::io::stdout();
    write!(stdout, "\x1b]11;?\x07").ok()?;
    stdout.flush().ok()?;

    // Read with timeout
    let start_time = Instant::now();
    let timeout = Duration::from_millis(50);
    let mut buffer = Vec::new();

    let mut poll_fd = libc::pollfd {
        fd: 0, // stdin
        events: libc::POLLIN,
        revents: 0,
    };

    while start_time.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(start_time.elapsed())
            .unwrap_or(Duration::ZERO);
        let ret = unsafe { libc::poll(&mut poll_fd, 1, remaining.as_millis() as libc::c_int) };
        if ret > 0 && (poll_fd.revents & libc::POLLIN) != 0 {
            let mut byte = 0u8;
            let n = unsafe { libc::read(0, &mut byte as *mut u8 as *mut libc::c_void, 1) };
            if n > 0 {
                buffer.push(byte);
                if byte == 0x07 {
                    break;
                }
                if buffer.len() >= 2
                    && buffer[buffer.len() - 2] == 0x1b
                    && buffer[buffer.len() - 1] == b'\\'
                {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let resp = String::from_utf8_lossy(&buffer);
    let (r, g, b) = parse_osc_response(&resp)?;
    let y = (2126 * r as u32 + 7152 * g as u32 + 722 * b as u32) / 10000;
    if y > 128 {
        Some(ThemeMode::Light)
    } else {
        Some(ThemeMode::Dark)
    }
}

#[cfg(not(unix))]
fn query_osc_11() -> Option<ThemeMode> {
    None
}

fn parse_osc_response(resp: &str) -> Option<(u8, u8, u8)> {
    let rgb_start = resp.find("rgb:")?;
    let content = &resp[rgb_start + 4..];
    let mut parts = Vec::new();
    let mut current = String::new();
    for c in content.chars() {
        if c.is_ascii_hexdigit() {
            current.push(c);
        } else if c == '/' {
            parts.push(current);
            current = String::new();
        } else {
            break;
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.len() != 3 {
        return None;
    }
    let mut rgb = [0u8; 3];
    for i in 0..3 {
        let part = &parts[i];
        if part.len() < 2 {
            return None;
        }
        rgb[i] = u8::from_str_radix(&part[0..2], 16).ok()?;
    }
    Some((rgb[0], rgb[1], rgb[2]))
}

pub fn no_color_active() -> bool {
    let is_test_env = cfg!(test)
        || std::env::current_exe()
            .map(|p| {
                p.parent()
                    .and_then(|parent| parent.file_name())
                    .map(|name| name == "deps")
                    .unwrap_or(false)
            })
            .unwrap_or(false);
    if is_test_env {
        std::env::var_os("MANTIS_TEST_NO_COLOR").is_some()
    } else {
        std::env::var_os("NO_COLOR").is_some() || std::env::var("TERM").as_deref() == Ok("dumb")
    }
}

#[cfg(test)]
#[path = "theme_test.rs"]
mod tests;
