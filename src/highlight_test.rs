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
    assert_eq!(result[0][0].0.add_modifier, Modifier::empty());
}

#[test]
fn highlight_rust_code_colors_keywords() {
    let h = Highlighter::new("base16-ocean.dark");
    let result = h.highlight(Path::new("main.rs"), &["fn main() {".to_string()]);
    assert!(
        result[0].len() > 1,
        "expected multiple spans for Rust code, got {}",
        result[0].len()
    );
    let has_fg = result[0].iter().any(|(s, _)| s.fg.is_some());
    assert!(has_fg, "Rust code should have some colored spans");
}

#[test]
fn highlight_state_tracks_across_lines() {
    let h = Highlighter::new("base16-ocean.dark");
    let lines = vec!["/// doc comment".to_string(), "fn main() {}".to_string()];
    let result = h.highlight(Path::new("main.rs"), &lines);
    assert_eq!(result.len(), 2);
    assert!(result[0][0].0.fg.is_some());
    assert!(result[1][0].0.fg.is_some());
}
