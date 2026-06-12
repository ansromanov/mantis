use ratatui::style::{Color, Modifier, Style};
use std::path::Path;
use syntect::{
    easy::HighlightLines,
    highlighting::{FontStyle, Style as SynStyle, ThemeSet},
    parsing::SyntaxSet,
};

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

fn syn_color(c: syntect::highlighting::Color) -> Color {
    if c.a == 0 {
        Color::Reset
    } else {
        Color::Rgb(c.r, c.g, c.b)
    }
}
