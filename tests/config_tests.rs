use std::fs;
use std::time::SystemTime;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use mantis::config::{self, pressed, Config, Keymap};
use mantis::theme::{Theme, ThemeConfig};

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[test]
fn default_keymap_has_expected_bindings() {
    let km = Keymap::default();
    assert!(pressed(
        &km.quit,
        &ev(KeyCode::Char('q'), KeyModifiers::empty())
    ));
    assert!(pressed(
        &km.quit,
        &ev(KeyCode::Char('c'), KeyModifiers::CONTROL)
    ));
    assert!(pressed(
        &km.switch_panel,
        &ev(KeyCode::Tab, KeyModifiers::empty())
    ));
    assert!(pressed(
        &km.toggle_hidden,
        &ev(KeyCode::Char('.'), KeyModifiers::empty())
    ));
}

#[test]
fn config_uses_serde_defaults_for_missing_fields() {
    let cfg: Config = toml::from_str("tree_width = 42").unwrap();
    assert_eq!(cfg.tree_width, 42);
    assert!(!cfg.show_hidden);
    assert!(pressed(
        &cfg.keys.quit,
        &ev(KeyCode::Char('q'), KeyModifiers::empty())
    ));
}

#[test]
fn config_rejects_invalid_key_spec() {
    let result: Result<Config, _> = toml::from_str("[keys]\nquit = [\"nope\"]");
    assert!(result.is_err());
}

#[test]
fn default_config_serializes_and_round_trips() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).expect("default config must serialize");
    assert!(!toml_str.is_empty());
    let back: Config = toml::from_str(&toml_str).expect("serialized config must round-trip");
    assert_eq!(back.show_hidden, cfg.show_hidden);
    assert_eq!(back.tree_width, cfg.tree_width);
    assert!(pressed(
        &back.keys.quit,
        &ev(KeyCode::Char('q'), KeyModifiers::empty())
    ));
}

#[test]
fn theme_config_with_name_serializes_and_round_trips() {
    let cfg = Config {
        theme: ThemeConfig::from_preset("monokai"),
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).expect("must serialize");
    assert!(
        toml_str.contains("monokai"),
        "theme name must appear in TOML:\n{toml_str}"
    );
    let back: Config = toml::from_str(&toml_str).expect("must round-trip");
    let theme = back.theme.resolve();
    let expected = Theme::load("monokai").unwrap();
    assert_eq!(
        theme.accent, expected.accent,
        "theme must be restored from name"
    );
}

#[test]
fn load_returns_path_of_loaded_file() {
    let dir = std::env::temp_dir().join(format!(
        "tv_cfg_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let cfg_file = dir.join("mantis.toml");
    fs::write(&cfg_file, "tree_width = 99\n").unwrap();

    let (cfg, path, err) = config::load(&dir);
    assert_eq!(cfg.tree_width, 99);
    assert_eq!(path.as_deref(), Some(cfg_file.as_path()));
    assert!(err.is_none());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_and_reload_preserves_theme() {
    let dir = std::env::temp_dir().join(format!(
        "tv_theme_test_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("mantis.toml"), "tree_width = 30\n").unwrap();

    let (mut cfg, path, _) = config::load(&dir);
    assert_eq!(cfg.tree_width, 30);

    cfg.theme = ThemeConfig::from_preset("synthwave84");
    config::save(&cfg, path.as_deref().unwrap()).unwrap();

    let (reloaded, _, _) = config::load(&dir);
    let theme = reloaded.theme.resolve();
    let expected = Theme::load("synthwave84").unwrap();
    assert_eq!(
        theme.accent, expected.accent,
        "theme must survive a save/reload cycle"
    );

    fs::remove_dir_all(&dir).ok();
}
