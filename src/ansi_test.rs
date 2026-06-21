use ratatui::style::{Color, Modifier, Style};

use super::*;

#[test]
fn plain_text_no_escapes() {
    let result = parse_ansi_line("hello world");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "hello world");
    assert_eq!(result[0].0, Style::default());
}

#[test]
fn empty_string() {
    let result = parse_ansi_line("");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "");
}

#[test]
fn single_colour() {
    let result = parse_ansi_line("\x1b[31mred\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn multiple_colours() {
    let result = parse_ansi_line("\x1b[31mred\x1b[32mgreen\x1b[0m");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].1, "green");
    assert_eq!(result[1].0.fg, Some(Color::Green));
}

#[test]
fn bold_and_colour() {
    let result = parse_ansi_line("\x1b[1;31mbold red\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "bold red");
    assert!(result[0].0.add_modifier & Modifier::BOLD == Modifier::BOLD);
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn cyan_heading() {
    let line = "\x1b[1;36m# Heading\x1b[0m";
    let result = parse_ansi_line(line);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "# Heading");
    assert!(result[0].0.add_modifier & Modifier::BOLD == Modifier::BOLD);
    assert_eq!(result[0].0.fg, Some(Color::Cyan));
}

#[test]
fn green_word_in_text() {
    let line = "Normal text with \x1b[32mgreen\x1b[0m word.";
    let result = parse_ansi_line(line);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].1, "Normal text with ");
    assert_eq!(result[0].0, Style::default());
    assert_eq!(result[1].1, "green");
    assert_eq!(result[1].0.fg, Some(Color::Green));
    assert_eq!(result[2].1, " word.");
    assert_eq!(result[2].0, Style::default());
}

#[test]
fn bright_colours() {
    let result = parse_ansi_line("\x1b[91mbright red\x1b[0m");
    assert_eq!(result[0].1, "bright red");
    assert_eq!(result[0].0.fg, Some(Color::LightRed));
}

#[test]
fn background_colour() {
    let result = parse_ansi_line("\x1b[41mred bg\x1b[0m");
    assert_eq!(result[0].1, "red bg");
    assert_eq!(result[0].0.bg, Some(Color::Red));
}

#[test]
fn underline() {
    let result = parse_ansi_line("\x1b[4munderline\x1b[0m");
    assert_eq!(result[0].1, "underline");
    assert!(result[0].0.add_modifier & Modifier::UNDERLINED == Modifier::UNDERLINED);
}

#[test]
fn non_sgr_sequence_is_stripped() {
    let result = parse_ansi_line("before\x1b[Aafter");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "beforeafter");
}

#[test]
fn adjacent_segments_same_style_are_merged() {
    let result = parse_ansi_line("\x1b[31mhello\x1b[31m world\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "hello world");
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn no_style_reset_between_colours() {
    let result = parse_ansi_line("\x1b[31mred\x1b[32mgreen");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].1, "green");
    assert_eq!(result[1].0.fg, Some(Color::Green));
}

#[test]
fn reset_between_text() {
    let result = parse_ansi_line("\x1b[31mred\x1b[0mnormal");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].1, "normal");
    assert_eq!(result[1].0, Style::default());
}

#[test]
fn no_trailing_empty_after_reset() {
    let result = parse_ansi_line("plain\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "plain");
}
