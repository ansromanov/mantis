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
fn default_theme_has_active_line_bg() {
    let t = Theme::load("default").expect("default theme must load");
    assert_ne!(t.active_line_bg, Color::Reset);
    assert_eq!(t.active_line_bg, Color::Rgb(0x3a, 0x5a, 0x5a));
}

#[test]
fn missing_active_line_bg_falls_back_to_selection_bg() {
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
    let t = Theme::from_toml(toml).expect("legacy theme (no active_line_bg) must parse");
    assert_eq!(t.active_line_bg, t.selection_bg);
    assert_eq!(t.active_line_bg, Color::Rgb(0x50, 0x50, 0x50));
}

#[test]
fn active_line_bg_override_via_theme_config() {
    let cfg = ThemeConfig {
        name: Some("default".into()),
        active_line_bg: Some("#ff0000".into()),
        ..Default::default()
    };
    let t = cfg.resolve();
    assert_eq!(t.active_line_bg, Color::Rgb(0xff, 0x00, 0x00));
    // Other fields must still come from the base theme
    assert_eq!(t.accent, Color::Cyan);
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
fn every_embedded_manifest_entry_parses() {
    for (name, toml) in super::EMBEDDED_MANIFEST {
        assert!(
            Theme::from_toml(toml).is_some(),
            "embedded theme \"{name}\" must parse successfully"
        );
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

#[test]
fn color_to_hex_rgb_roundtrips() {
    assert_eq!(color_to_hex(Color::Rgb(0xaa, 0xbb, 0xcc)), "#aabbcc");
}

#[test]
fn color_to_hex_named_colors_are_stable() {
    assert_eq!(color_to_hex(Color::White), "#ffffff");
    assert_eq!(color_to_hex(Color::Black), "#000000");
    assert_eq!(color_to_hex(Color::LightCyan), "#00ffff");
}

#[test]
fn color_to_hex_reset_and_indexed_fall_back_to_gray() {
    assert_eq!(color_to_hex(Color::Reset), "#7f7f7f");
    assert_eq!(color_to_hex(Color::Indexed(42)), "#7f7f7f");
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
//
// These tests mutate XDG_CONFIG_HOME to redirect to a temporary directory so
// they never touch the developer's real config. ENV_LOCK serialises the three
// tests so concurrent env-var reads from other threads don't race.
// ---------------------------------------------------------------------------

use std::sync::MutexGuard;

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Redirects the platform config-home env var to a fresh temp dir for the
/// duration of `f`, then restores the original value and removes the temp dir.
/// On Windows this is APPDATA; everywhere else it is XDG_CONFIG_HOME.
/// Uses the shared ENV_LOCK from plugin.rs so this serialises against
/// plugin_test.rs, which also mutates the same env var.
fn with_isolated_config_home<F: FnOnce(&std::path::Path)>(f: F) {
    let _guard: MutexGuard<()> = crate::plugin::ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("tv2_theme_test_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();

    // SAFETY: ENV_LOCK serialises all callers; no other thread mutates this var.
    #[cfg(windows)]
    let env_key = "APPDATA";
    #[cfg(not(windows))]
    let env_key = "XDG_CONFIG_HOME";

    let old = std::env::var_os(env_key);
    unsafe { std::env::set_var(env_key, &tmp) };

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&tmp)));

    unsafe {
        match old {
            Some(v) => std::env::set_var(env_key, v),
            None => std::env::remove_var(env_key),
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);

    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}

#[test]
fn install_embedded_themes_creates_theme_files() {
    with_isolated_config_home(|tmp| {
        install_embedded_themes();
        let themes_dir = tmp.join("mantis").join("themes");
        assert!(themes_dir.is_dir(), "themes directory should be created");
        assert!(
            themes_dir.join("default.toml").exists(),
            "default.toml must be installed"
        );
        assert!(
            themes_dir.join("monokai.toml").exists(),
            "monokai.toml must be installed"
        );
    });
}

#[test]
fn discover_all_with_existing_user_dir_covers_user_theme_loop() {
    with_isolated_config_home(|_tmp| {
        install_embedded_themes();
        let themes = Theme::discover_all();
        let names: Vec<&str> = themes.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"default"));
        assert!(names.contains(&"monokai"));
        assert!(themes.len() >= 9);
    });
}

#[test]
fn discover_all_user_theme_extends_list() {
    with_isolated_config_home(|tmp| {
        let themes_dir = tmp.join("mantis").join("themes");
        std::fs::create_dir_all(&themes_dir).unwrap();
        let test_name = "custom-test-theme";
        std::fs::write(
            themes_dir.join(format!("{test_name}.toml")),
            include_str!("../themes/default.toml"),
        )
        .unwrap();

        let themes = Theme::discover_all();
        assert!(
            themes.iter().any(|(n, _)| n == test_name),
            "user-added theme must appear in discover_all output"
        );
    });
}

#[test]
fn monochrome_theme_is_all_reset() {
    let t = Theme::monochrome();
    assert_eq!(t.background, Color::Reset);
    assert_eq!(t.accent, Color::Reset);
    assert_eq!(t.selection_bg, Color::Reset);
    assert!(t.is_monochrome());
    assert_eq!(
        t.selection_style(),
        ratatui::style::Style::default().add_modifier(ratatui::style::Modifier::REVERSED)
    );
}

#[test]
fn parse_osc_response_rgb() {
    assert_eq!(parse_osc_response("11;rgb:0000/0000/0000"), Some((0, 0, 0)));
    assert_eq!(
        parse_osc_response("11;rgb:ffff/ffff/ffff"),
        Some((255, 255, 255))
    );
    assert_eq!(parse_osc_response("11;rgb:12/34/56"), Some((18, 52, 86)));
    assert_eq!(parse_osc_response("invalid"), None);
}

#[test]
fn colorfgbg_parsing() {
    std::env::set_var("COLORFGBG", "15;0");
    assert_eq!(get_colorfgbg_background(), Some(ThemeMode::Dark));

    std::env::set_var("COLORFGBG", "15;7");
    assert_eq!(get_colorfgbg_background(), Some(ThemeMode::Light));

    std::env::set_var("COLORFGBG", "15;245");
    assert_eq!(get_colorfgbg_background(), Some(ThemeMode::Light));

    std::env::remove_var("COLORFGBG");
}
