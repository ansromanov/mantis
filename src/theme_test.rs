use super::*;

#[test]
fn default_theme_loads_from_embedded() {
    let t = Theme::load("default").expect("default theme must load");
    assert_eq!(t.background, Color::Reset);
    assert_eq!(t.accent, Color::Cyan);
}

#[test]
fn bundled_default_has_distinct_git_state_colors() {
    let toml = include_str!("../themes/default.toml");
    let t = Theme::from_toml(toml).expect("bundled default must parse");
    assert_eq!(t.git_clean, Color::Green);
    assert_eq!(t.git_dirty, Color::Yellow);
    assert_eq!(t.git_conflict, Color::Red);
    assert_eq!(t.git_progress, Color::Rgb(0xff, 0x87, 0x00));
}

#[test]
fn missing_git_state_fields_fall_back_to_existing_roles() {
    let toml = r##"
        background = "reset"
        accent = "cyan"
        accent_alt = "yellow"
        dim = "darkgray"
        text = "white"
        dir = "blue"
        file = "reset"
        selection_bg = "#505050"
        selection_fg = "yellow"
        heading1 = "lightcyan"
        heading2 = "lightyellow"
        heading3 = "lightgreen"
        code = "lightyellow"
        diff_add = "green"
        diff_del = "red"
        git_clean = "green"
        git_dirty = "yellow"
        syntax = "base16-ocean.dark"
    "##;
    let t = Theme::from_toml(toml).expect("legacy theme must still parse");
    assert_eq!(t.git_conflict, t.diff_del);
    assert_eq!(t.git_progress, t.git_dirty);
}

#[test]
fn unknown_name_returns_none() {
    assert!(Theme::load("nonexistent-theme").is_none());
}

#[test]
fn all_embedded_themes_are_valid() {
    let themes = Theme::discover_all();
    assert!(themes.len() >= 9, "should have at least 9 built-in themes");
    for (name, _) in &themes {
        assert!(!name.is_empty(), "each theme must have a non-empty name");
    }
}

#[test]
fn named_preset_is_the_base_and_overrides_layer_on_top() {
    let cfg = ThemeConfig {
        name: Some("monokai".into()),
        accent: Some("#000000".into()),
        ..Default::default()
    };
    let t = cfg.resolve();
    let monokai = Theme::load("monokai").unwrap();
    assert_eq!(t.accent, Color::Rgb(0, 0, 0));
    assert_eq!(t.diff_del, monokai.diff_del);
    assert_eq!(t.syntax, monokai.syntax);
}

#[test]
fn background_defaults_transparent_but_presets_set_it() {
    assert_eq!(Theme::load("default").unwrap().background, Color::Reset);
    assert_eq!(
        Theme::load("monokai").unwrap().background,
        Color::Rgb(0x27, 0x28, 0x22)
    );
    let cfg = ThemeConfig {
        name: Some("monokai".into()),
        transparent_background: Some(true),
        ..Default::default()
    };
    assert_eq!(cfg.resolve().background, Color::Reset);
}

#[test]
fn default_is_used_for_unset_and_invalid() {
    let cfg = ThemeConfig {
        accent: Some("#010203".into()),
        dim: Some("not-a-color".into()),
        ..Default::default()
    };
    let t = cfg.resolve();
    assert_eq!(t.accent, Color::Rgb(1, 2, 3));
    assert_eq!(t.dim, Theme::default().dim);
    assert_eq!(t.diff_add, Theme::default().diff_add);
}

#[test]
fn discover_all_includes_synthwave84() {
    let themes = Theme::discover_all();
    assert!(
        themes.iter().any(|(n, _)| n == "synthwave84"),
        "synthwave84 must be in discovered themes"
    );
}
