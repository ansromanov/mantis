use ratatui::style::{Color, Modifier, Style};
use std::path::Path;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SynStyle, ThemeSet},
    parsing::SyntaxSet,
};

/// Wraps a syntect `SyntaxSet` + `ThemeSet` to provide syntax highlighting
/// for file contents. Compiled once at startup and reused across file opens.
pub struct Highlighter {
    ss: SyntaxSet,
    ts: ThemeSet,
    theme: String,
}

impl Highlighter {
    /// Builds a highlighter using the named syntect theme, falling back to
    /// `base16-ocean.dark` if the name is unknown.
    pub fn new(theme: &str) -> Self {
        let ts = ThemeSet::load_defaults();
        let theme = if ts.themes.contains_key(theme) {
            theme.to_string()
        } else {
            "base16-ocean.dark".to_string()
        };
        Highlighter {
            ss: SyntaxSet::load_defaults_nonewlines(),
            ts,
            theme,
        }
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
