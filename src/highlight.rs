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
        let syntax = self
            .ss
            .find_syntax_for_file(path)
            .ok()
            .flatten()
            .unwrap_or_else(|| self.ss.find_syntax_plain_text());

        let theme = &self.ts.themes[&self.theme];
        let mut h = HighlightLines::new(syntax, theme);

        lines
            .iter()
            .map(|line| match h.highlight_line(line, &self.ss) {
                Ok(regions) => regions
                    .into_iter()
                    .map(|(s, text)| (to_ratatui(s), text.to_owned()))
                    .collect(),
                Err(_) => vec![(Style::default(), line.clone())],
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
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn new_with_valid_theme() {
        let h = Highlighter::new("base16-ocean.dark");
        assert_eq!(h.theme, "base16-ocean.dark");
    }

    #[test]
    fn new_falls_back_for_unknown_theme() {
        let h = Highlighter::new("nonexistent-theme-name");
        assert_eq!(h.theme, "base16-ocean.dark");
    }

    #[test]
    fn highlight_returns_one_vec_per_line() {
        let h = Highlighter::new("base16-ocean.dark");
        let lines = vec!["hello".to_string(), "world".to_string()];
        let result = h.highlight(Path::new("f.txt"), &lines);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0][0].1, "hello");
        assert_eq!(result[1][0].1, "world");
    }

    #[test]
    fn highlight_plain_text_no_extra_styling() {
        let h = Highlighter::new("base16-ocean.dark");
        let result = h.highlight(Path::new("f.txt"), &[":)".to_string()]);
        assert_eq!(result[0][0].1, ":)");
        // Plain text gets a single span (theme default foreground, no modifiers).
        assert_eq!(result[0][0].0.add_modifier, Modifier::empty());
    }

    #[test]
    fn highlight_rust_code_colors_keywords() {
        let h = Highlighter::new("base16-ocean.dark");
        let result = h.highlight(Path::new("main.rs"), &["fn main() {".to_string()]);
        // Rust code should emit multiple styled spans (keyword, ident, punct…).
        assert!(
            result[0].len() > 1,
            "expected multiple spans for Rust code, got {}",
            result[0].len()
        );
        // At least one span has a non-default foreground.
        let has_fg = result[0].iter().any(|(s, _)| s.fg.is_some());
        assert!(has_fg, "Rust code should have some colored spans");
    }

    #[test]
    fn highlight_state_tracks_across_lines() {
        let h = Highlighter::new("base16-ocean.dark");
        let lines = vec!["/// doc comment".to_string(), "fn main() {}".to_string()];
        let result = h.highlight(Path::new("main.rs"), &lines);
        assert_eq!(result.len(), 2);
        // Doc comment lines should have a span (likely green in ocean theme).
        assert!(result[0][0].0.fg.is_some());
        // Second line should have keyword coloring for `fn`.
        assert!(result[1][0].0.fg.is_some());
    }
}
