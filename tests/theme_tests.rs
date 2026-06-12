use ratatui::style::Color;

use tree_viewer::theme::{parse_color, Theme, PRESETS};

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
fn all_listed_presets_resolve() {
    for name in PRESETS {
        assert!(Theme::preset(name).is_some(), "missing preset {name}");
    }
    assert!(Theme::preset("bogus").is_none());
}
