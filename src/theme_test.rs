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

// ---------------------------------------------------------------------------
// parse_color coverage — named colors not hit by embedded theme TOML files
// ---------------------------------------------------------------------------

#[test]
fn parse_color_black() {
    assert_eq!(parse_color("black"), Some(Color::Black));
}

#[test]
fn parse_color_gray_and_aliases() {
    assert_eq!(parse_color("gray"), Some(Color::Gray));
    assert_eq!(parse_color("grey"), Some(Color::Gray));
    assert_eq!(parse_color("darkgrey"), Some(Color::DarkGray));
}

#[test]
fn parse_color_remaining_light_variants() {
    assert_eq!(parse_color("lightred"), Some(Color::LightRed));
    assert_eq!(parse_color("lightblue"), Some(Color::LightBlue));
    assert_eq!(parse_color("lightmagenta"), Some(Color::LightMagenta));
}

#[test]
fn parse_color_magenta() {
    assert_eq!(parse_color("magenta"), Some(Color::Magenta));
}

#[test]
fn parse_color_hex_valid() {
    assert_eq!(parse_color("#aabbcc"), Some(Color::Rgb(0xaa, 0xbb, 0xcc)));
    assert_eq!(parse_color(" #001122 "), Some(Color::Rgb(0x00, 0x11, 0x22)));
}

#[test]
fn parse_color_hex_wrong_length_returns_none() {
    assert!(parse_color("#12345").is_none());
    assert!(parse_color("#1234567").is_none());
    assert!(parse_color("#").is_none());
}

#[test]
fn parse_color_invalid_returns_none() {
    assert!(parse_color("").is_none());
    assert!(parse_color("not-a-color").is_none());
    assert!(parse_color("rgb(1,2,3)").is_none());
}

// ---------------------------------------------------------------------------
// Theme::from_toml error paths
// ---------------------------------------------------------------------------

#[test]
fn theme_from_toml_invalid_color_returns_none() {
    let toml = r##"
        background = "not-a-color"
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
    assert!(Theme::from_toml(toml).is_none());
}

#[test]
fn theme_from_toml_malformed_toml_returns_none() {
    assert!(Theme::from_toml("not toml at all :::").is_none());
}

// ---------------------------------------------------------------------------
// ThemeConfig::from_preset
// ---------------------------------------------------------------------------

#[test]
fn theme_config_from_preset_sets_name_and_no_overrides() {
    let cfg = ThemeConfig::from_preset("monokai");
    assert_eq!(cfg.name, Some("monokai".to_string()));
    assert!(cfg.accent.is_none());
    assert!(cfg.dim.is_none());
    let t = cfg.resolve();
    let monokai = Theme::load("monokai").unwrap();
    assert_eq!(t.syntax, monokai.syntax);
    assert_eq!(t.diff_del, monokai.diff_del);
}

// ---------------------------------------------------------------------------
// install_embedded_themes + discover_all user-themes code path
// ---------------------------------------------------------------------------

#[test]
fn install_embedded_themes_creates_theme_files() {
    install_embedded_themes();
    let dir = user_themes_dir().expect("must have a themes dir on this platform");
    assert!(dir.is_dir(), "themes directory should exist after install");
    assert!(
        dir.join("default.toml").exists(),
        "default.toml must be installed"
    );
    assert!(
        dir.join("monokai.toml").exists(),
        "monokai.toml must be installed"
    );
}

#[test]
fn discover_all_with_existing_user_dir_covers_user_theme_loop() {
    // Ensure the user themes dir has at least the embedded themes so the
    // user-theme loop in discover_all executes.
    install_embedded_themes();
    let themes = Theme::discover_all();
    let names: Vec<&str> = themes.iter().map(|(n, _)| n.as_str()).collect();
    assert!(names.contains(&"default"));
    assert!(names.contains(&"monokai"));
    assert!(themes.len() >= 9);
}

#[test]
fn discover_all_user_theme_extends_list() {
    let Some(dir) = user_themes_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let test_name = "__tv2_test_ext__";
    let path = dir.join(format!("{test_name}.toml"));
    std::fs::write(&path, include_str!("../themes/default.toml")).unwrap();

    let themes = Theme::discover_all();
    let found = themes.iter().any(|(n, _)| n == test_name);
    let _ = std::fs::remove_file(&path);

    assert!(found, "user-added theme must appear in discover_all output");
}
