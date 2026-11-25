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

use std::fs;

use ratatui::style::Color;
use serde::Deserialize;

use crate::code_intel::DiagnosticSeverity;

/// The valid semantic color-role names accepted by feature metadata that
/// references theme roles by name (e.g. the `[statusbar.colors]` overrides).
/// Every entry maps to a `Theme` color field; `syntax` is deliberately absent
/// because it names a syntect theme rather than a color.
pub const THEME_COLOR_ROLES: &[&str] = &[
    "background",
    "accent",
    "accent_alt",
    "dim",
    "text",
    "dir",
    "file",
    "selection_bg",
    "selection_fg",
    "heading1",
    "heading2",
    "heading3",
    "code",
    "diff_add",
    "diff_del",
    "git_clean",
    "git_dirty",
    "git_conflict",
    "git_progress",
    "breadcrumb_fg",
    "breadcrumb_bg",
    "active_line_bg",
];

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
    /// Color role used for diagnostics at the given severity.
    pub fn diagnostic_color(&self, severity: DiagnosticSeverity) -> Color {
        match severity {
            DiagnosticSeverity::Error => self.git_conflict,
            DiagnosticSeverity::Warning => self.git_progress,
            DiagnosticSeverity::Info => self.accent,
            DiagnosticSeverity::Hint => self.dim,
        }
    }

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

    /// Resolves a semantic color-role name (e.g. `"accent"` or `"git_dirty"`)
    /// to its themed color. Used by status-bar per-segment overrides. Returns
    /// `None` for unknown role names.
    pub fn role_color(&self, role: &str) -> Option<Color> {
        let c = match role {
            "background" => self.background,
            "accent" => self.accent,
            "accent_alt" => self.accent_alt,
            "dim" => self.dim,
            "text" => self.text,
            "dir" => self.dir,
            "file" => self.file,
            "selection_bg" => self.selection_bg,
            "selection_fg" => self.selection_fg,
            "heading1" => self.heading1,
            "heading2" => self.heading2,
            "heading3" => self.heading3,
            "code" => self.code,
            "diff_add" => self.diff_add,
            "diff_del" => self.diff_del,
            "git_clean" => self.git_clean,
            "git_dirty" => self.git_dirty,
            "git_conflict" => self.git_conflict,
            "git_progress" => self.git_progress,
            "breadcrumb_fg" => self.breadcrumb_fg,
            "breadcrumb_bg" => self.breadcrumb_bg,
            "active_line_bg" => self.active_line_bg,
            _ => return None,
        };
        Some(c)
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

mod background;
mod color;
mod config;
mod embedded;
mod no_color;

pub use background::{detect_terminal_background, get_detected_background, ThemeMode};
#[cfg(test)]
use background::{get_colorfgbg_background, parse_osc_response};
pub use color::{color_to_hex, parse_color};
pub use config::ThemeConfig;
pub use embedded::install_embedded_themes;
use embedded::{all_embedded, user_themes_dir, EMBEDDED_MANIFEST};
#[cfg(test)]
pub use no_color::lock_no_color_test_env;
pub use no_color::no_color_active;

#[cfg(test)]
#[path = "theme_test.rs"]
mod tests;
