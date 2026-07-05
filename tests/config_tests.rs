use std::fs;
use std::path::PathBuf;
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
    let mut cfg: Config = toml::from_str("tree_width = 42").unwrap();
    cfg.migrate_legacy_flat_fields();
    assert_eq!(cfg.tree.width, 42);
    assert!(!cfg.tree.show_hidden);
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
    assert_eq!(back.tree.show_hidden, cfg.tree.show_hidden);
    assert_eq!(back.tree.width, cfg.tree.width);
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
    assert_eq!(cfg.tree.width, 99);
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
    assert_eq!(cfg.tree.width, 30);

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

#[test]
fn config_schema_snapshot_is_up_to_date() {
    let snapshot_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("config_schema.snapshot");
    let actual = mantis::config::schema_paths();
    let actual_text = actual.join("\n") + "\n";

    if std::env::var("UPDATE_SNAPSHOT").as_deref() == Ok("1") {
        fs::write(&snapshot_path, &actual_text).unwrap_or_else(|e| {
            panic!("failed to write snapshot {}: {e}", snapshot_path.display())
        });
        return;
    }

    let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|e| {
        panic!(
            "snapshot file not found at {}: {e}\n\
             Run `UPDATE_SNAPSHOT=1 cargo test config_schema_snapshot --test config_tests` \
             to generate it.",
            snapshot_path.display()
        )
    });

    // Filter comment lines from the snapshot for comparison.
    let expected_paths: Vec<&str> = expected
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    let actual_paths: Vec<&str> = actual.iter().map(String::as_str).collect();

    if expected_paths != actual_paths {
        let only_in_expected: Vec<&str> = expected_paths
            .iter()
            .filter(|p| !actual_paths.contains(p))
            .copied()
            .collect();
        let only_in_actual: Vec<&str> = actual_paths
            .iter()
            .filter(|p| !expected_paths.contains(p))
            .copied()
            .collect();

        let mut msg = String::new();
        msg.push_str("Config schema has changed!\n\n");
        if !only_in_expected.is_empty() {
            msg.push_str(&format!(
                "Paths REMOVED from schema (must add legacy migration or DEPRECATED_KEYS entry):\n  {}\n\n",
                only_in_expected.join("\n  ")
            ));
        }
        if !only_in_actual.is_empty() {
            msg.push_str(&format!(
                "Paths ADDED to schema:\n  {}\n\n",
                only_in_actual.join("\n  ")
            ));
        }
        msg.push_str(
            "To update the snapshot, run:\n  \
             UPDATE_SNAPSHOT=1 cargo test config_schema_snapshot --test config_tests\n\n\
             If you REMOVED a path, you MUST also:\n  \
             1. Add a legacy_* migration field in src/config/types.rs or src/config/keymap.rs\n  \
             2. Add the old path to DEPRECATED_KEYS in src/config/validate.rs\n  \
             3. Ensure the old path is accepted without warning by validate_keys",
        );

        panic!("{msg}");
    }
}
