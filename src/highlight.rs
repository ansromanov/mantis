//! Syntax highlighting via `syntect`, adapted to ratatui styles.
//!
//! `Highlighter` wraps a `syntect` `SyntaxSet` and `ThemeSet`, both compiled
//! once at startup and reused across every file open. It picks a syntax by file
//! extension or first line, highlights a file line by line, and converts each
//! `syntect` style span into a ratatui `(Style, String)` pair - translating
//! colors and font flags (bold/italic/underline) into `ratatui::style`. The
//! named theme is resolved on construction with a fallback to
//! `base16-ocean.dark` when unknown, and can be swapped when the user changes
//! themes. Its output feeds both the synchronous and background file-load paths.
//!
//! Extra syntax definitions (e.g. from plugins) can be passed via
//! [`with_extra_syntaxes`]; each is a `.sublime-syntax` file loaded into the
//! `SyntaxSet` so its file extensions are recognised during highlighting.

use ratatui::style::{Color, Modifier, Style};
use std::fs;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SynStyle, ThemeSet},
    parsing::{SyntaxDefinition, SyntaxSet},
};

use crate::plugin::ExtraSyntax;

/// Wraps a syntect `SyntaxSet` + `ThemeSet` to provide syntax highlighting
/// for file contents. Compiled once at startup and reused across file opens.
pub struct Highlighter {
    ss: Arc<SyntaxSet>,
    ts: Arc<ThemeSet>,
    theme: String,
}

/// Returns the process-wide default `SyntaxSet`, loading it on first call.
fn default_ss() -> &'static Arc<SyntaxSet> {
    static SS: OnceLock<Arc<SyntaxSet>> = OnceLock::new();
    SS.get_or_init(|| Arc::new(SyntaxSet::load_defaults_nonewlines()))
}

/// Returns the process-wide default `ThemeSet`, loading it on first call.
fn default_ts() -> &'static Arc<ThemeSet> {
    static TS: OnceLock<Arc<ThemeSet>> = OnceLock::new();
    TS.get_or_init(|| Arc::new(ThemeSet::load_defaults()))
}

impl Highlighter {
    /// Builds a highlighter with extra syntax definitions loaded from plugins.
    /// Each [`ExtraSyntax`] provides a `.sublime-syntax` file path that is
    /// loaded into the `SyntaxSet` so syntect recognises its file extensions.
    ///
    /// When `extra` is empty the process-wide cached `SyntaxSet` and `ThemeSet`
    /// are reused (Arc clone), so repeated construction is nearly free.
    pub fn with_extra_syntaxes(theme: &str, extra: &[ExtraSyntax]) -> Self {
        let ts = default_ts().clone();
        let theme = if ts.themes.contains_key(theme) {
            theme.to_string()
        } else {
            "base16-ocean.dark".to_string()
        };
        let ss = if extra.is_empty() {
            default_ss().clone()
        } else {
            let mut builder = (**default_ss()).clone().into_builder();
            for extra_syn in extra {
                if let Ok(s) = fs::read_to_string(&extra_syn.syntax_path) {
                    if let Ok(def) = SyntaxDefinition::load_from_str(
                        &s,
                        false,
                        extra_syn.syntax_path.file_stem().and_then(|n| n.to_str()),
                    ) {
                        builder.add(def);
                    }
                }
            }
            Arc::new(builder.build())
        };
        Highlighter { ss, ts, theme }
    }

    /// Syntax-highlights the given lines by detecting the file type from
    /// `path` and applying the configured syntect theme. Returns one Vec of
    /// styled spans per line. Unrecognized files get plain-text style.
    pub fn highlight(&self, path: &Path, lines: &[String]) -> Vec<Vec<(Style, String)>> {
        self.highlight_impl(path, lines.iter().map(|s| s.as_str()))
    }

    /// Syntax-highlights a range of lines (as `&str` slices) for virtualization.
    /// Same as `highlight` but accepts a slice of borrowed strings to avoid
    /// allocating a `Vec<String>` for the visible window.
    pub fn highlight_range(&self, path: &Path, lines: &[&str]) -> Vec<Vec<(Style, String)>> {
        self.highlight_impl(path, lines.iter().copied())
    }

    fn highlight_impl<'a>(
        &self,
        path: &Path,
        lines: impl Iterator<Item = &'a str>,
    ) -> Vec<Vec<(Style, String)>> {
        let syntax = self
            .ss
            .find_syntax_for_file(path)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.ss.find_syntax_plain_text());

        let theme = &self.ts.themes[&self.theme];
        let mut h = HighlightLines::new(syntax, theme);

        lines
            .map(|line| match h.highlight_line(line, &self.ss) {
                Ok(regions) => regions
                    .into_iter()
                    .map(|(s, text)| (to_ratatui(s), text.to_owned()))
                    .collect(),
                Err(_) => vec![(Style::default(), line.to_owned())],
            })
            .collect()
    }
}

/// Converts a syntect style (foreground + font-style flags) into ratatui
/// Style with corresponding modifiers (bold, italic, underlined).
fn to_ratatui(s: SynStyle) -> Style {
    let mut style = Style::default().fg(syn_color(s.foreground));
    if s.font_style.contains(FontStyle::BOLD) {
        style = style.add_modifier(Modifier::BOLD);
    }
    if s.font_style.contains(FontStyle::ITALIC) {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if s.font_style.contains(FontStyle::UNDERLINE) {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

/// Converts a syntect color to ratatui Color. An alpha of 0 (transparent)
/// maps to `Color::Reset` so the terminal default shows through.
fn syn_color(c: syntect::highlighting::Color) -> Color {
    if c.a == 0 {
        Color::Reset
    } else {
        Color::Rgb(c.r, c.g, c.b)
    }
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
