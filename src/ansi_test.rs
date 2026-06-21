use super::*;
use ratatui::style::Color;

#[test]
fn plain_text_no_ansi() {
    let result = parse_ansi_line("hello world");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "hello world");
    assert_eq!(result[0].0, Style::default());
}

#[test]
fn empty_string() {
    assert!(parse_ansi_line("").is_empty());
}

#[test]
fn single_red_word() {
    let result = parse_ansi_line("\x1b[31mred\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn bold_and_red() {
    let result = parse_ansi_line("\x1b[1;31mbold red\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "bold red");
    assert!(result[0].0.add_modifier.contains(Modifier::BOLD));
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn multi_span_with_reset() {
    let result = parse_ansi_line("\x1b[31mred\x1b[0mnormal\x1b[32mgreen\x1b[0m");
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].1, "red");
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].1, "normal");
    assert_eq!(result[1].0, Style::default());
    assert_eq!(result[2].1, "green");
    assert_eq!(result[2].0.fg, Some(Color::Green));
}

#[test]
fn nested_styles_merge() {
    let result = parse_ansi_line("\x1b[1m\x1b[31mbold red\x1b[0m");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "bold red");
    assert!(result[0].0.add_modifier.contains(Modifier::BOLD));
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn bold_removal_works() {
    let result = parse_ansi_line("\x1b[1mbold\x1b[22mnormal");
    assert_eq!(result.len(), 2);
    assert!(result[0].0.add_modifier.contains(Modifier::BOLD));
    assert!(!result[1].0.add_modifier.contains(Modifier::BOLD));
}

#[test]
fn italic() {
    let result = parse_ansi_line("\x1b[3mitalic\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::ITALIC));
}

#[test]
fn underline() {
    let result = parse_ansi_line("\x1b[4munderlined\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::UNDERLINED));
}

#[test]
fn strikethrough() {
    let result = parse_ansi_line("\x1b[9mstruck\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::CROSSED_OUT));
}

#[test]
fn dim() {
    let result = parse_ansi_line("\x1b[2mdim\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::DIM));
}

#[test]
fn reversed() {
    let result = parse_ansi_line("\x1b[7mreversed\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::REVERSED));
}

#[test]
fn hidden() {
    let result = parse_ansi_line("\x1b[8mhidden\x1b[0m");
    assert!(result[0].0.add_modifier.contains(Modifier::HIDDEN));
}

#[test]
fn foreground_reset() {
    let result = parse_ansi_line("\x1b[31mred\x1b[39mdefault");
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].0.fg, None);
}

#[test]
fn background_reset() {
    let result = parse_ansi_line("\x1b[41mred_bg\x1b[49mdefault_bg");
    assert_eq!(result[0].0.bg, Some(Color::Red));
    assert_eq!(result[1].0.bg, None);
}

#[test]
fn background_color() {
    let result = parse_ansi_line("\x1b[42mgreen_bg\x1b[0m");
    assert_eq!(result[0].0.bg, Some(Color::Green));
}

#[test]
fn bright_foreground() {
    let result = parse_ansi_line("\x1b[91mbright_red\x1b[0m");
    assert_eq!(result[0].0.fg, Some(Color::LightRed));
}

#[test]
fn bright_background() {
    let result = parse_ansi_line("\x1b[101mbright_red_bg\x1b[0m");
    assert_eq!(result[0].0.bg, Some(Color::LightRed));
}

#[test]
fn indexed_256_color() {
    let result = parse_ansi_line("\x1b[38;5;82mindexed\x1b[0m");
    assert_eq!(result[0].0.fg, Some(Color::Indexed(82)));
}

#[test]
fn indexed_256_background() {
    let result = parse_ansi_line("\x1b[48;5;196mcolored_bg\x1b[0m");
    assert_eq!(result[0].0.bg, Some(Color::Indexed(196)));
}

#[test]
fn true_color_foreground() {
    let result = parse_ansi_line("\x1b[38;2;255;128;0morange\x1b[0m");
    assert_eq!(result[0].0.fg, Some(Color::Rgb(255, 128, 0)));
}

#[test]
fn true_color_background() {
    let result = parse_ansi_line("\x1b[48;2;100;200;150mbg\x1b[0m");
    assert_eq!(result[0].0.bg, Some(Color::Rgb(100, 200, 150)));
}

#[test]
fn reset_all_clears_style() {
    let result = parse_ansi_line("\x1b[1;31mbold_red\x1b[0mplain");
    assert_eq!(result.len(), 2);
    assert!(result[0].0.add_modifier.contains(Modifier::BOLD));
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].0, Style::default());
}

#[test]
fn sgr_without_number_is_reset() {
    let result = parse_ansi_line("\x1b[31mred\x1b[mplain");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0.fg, Some(Color::Red));
    assert_eq!(result[1].0, Style::default());
}

#[test]
fn unknown_codes_are_stripped() {
    let result = parse_ansi_line("\x1b[?25lhidden\x1b[?25h");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "hidden");
    // Non-SGR escape sequences should be silently consumed
}

#[test]
fn non_sgr_escape_stripped() {
    // CSI sequences that don't end in 'm' should be stripped without affecting style.
    let result = parse_ansi_line("abc\x1b[2Kdef");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "abcdef");
}

#[test]
fn multi_style_without_resets() {
    // Each SGR sequence creates a style boundary (flush point).
    let result = parse_ansi_line("\x1b[1mbold \x1b[31mred bold\x1b[0m");
    assert_eq!(result.len(), 2);
    assert!(result[0].0.add_modifier.contains(Modifier::BOLD));
    assert_eq!(result[0].0.fg, None);
    assert_eq!(result[0].1, "bold ");
    assert!(result[1].0.add_modifier.contains(Modifier::BOLD));
    assert_eq!(result[1].0.fg, Some(Color::Red));
    assert_eq!(result[1].1, "red bold");
}

#[test]
fn no_trailing_reset() {
    let result = parse_ansi_line("\x1b[31mred text");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "red text");
    assert_eq!(result[0].0.fg, Some(Color::Red));
}

#[test]
fn ansi_at_start_only() {
    // Without a reset the style continues for the whole string.
    let result = parse_ansi_line("\x1b[32mgreen all the way");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].1, "green all the way");
    assert_eq!(result[0].0.fg, Some(Color::Green));
}
