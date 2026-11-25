//! `ThemeConfig`: the `[theme]` section of `mantis.toml` and its resolution onto a preset.

use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use super::{get_detected_background, no_color_active, parse_color, Theme, ThemeMode};

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
