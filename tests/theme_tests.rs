use ratatui::style::Color;

use mantis::theme::{parse_color, Theme};

#[test]
fn parses_names_and_hex() {
    assert_eq!(parse_color("cyan"), Some(Color::Cyan));
    assert_eq!(parse_color("LightYellow"), Some(Color::LightYellow));
    assert_eq!(parse_color(" reset "), Some(Color::Reset));
    assert_eq!(parse_color("#ff8800"), Some(Color::Rgb(255, 136, 0)));
    assert_eq!(parse_color("nonsense"), None);
    assert_eq!(parse_color("#fff"), None);
}

#[test]
fn all_embedded_themes_resolve() {
    for (name, _) in Theme::discover_all() {
        assert!(Theme::load(&name).is_some(), "missing theme {name}");
    }
    assert!(Theme::load("bogus").is_none());
}
