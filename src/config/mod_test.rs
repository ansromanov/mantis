use super::*;

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[test]
fn parses_single_char_preserving_case() {
    let g = parse_binding("G").unwrap();
    assert_eq!(g.code, KeyCode::Char('G'));
    assert!(!g.ctrl && !g.alt);

    let lower = parse_binding("g").unwrap();
    assert_eq!(lower.code, KeyCode::Char('g'));
}

#[test]
fn parses_named_keys_case_insensitively() {
    assert_eq!(parse_binding("Up").unwrap().code, KeyCode::Up);
    assert_eq!(parse_binding("up").unwrap().code, KeyCode::Up);
    assert_eq!(parse_binding("PAGEUP").unwrap().code, KeyCode::PageUp);
    assert_eq!(parse_binding("enter").unwrap().code, KeyCode::Enter);
    assert_eq!(parse_binding("return").unwrap().code, KeyCode::Enter);
    assert_eq!(parse_binding("esc").unwrap().code, KeyCode::Esc);
    assert_eq!(parse_binding("space").unwrap().code, KeyCode::Char(' '));
}

#[test]
fn parses_modifiers() {
    let c = parse_binding("ctrl+c").unwrap();
    assert_eq!(c.code, KeyCode::Char('c'));
    assert!(c.ctrl && !c.alt);

    let dot = parse_binding("alt+.").unwrap();
    assert_eq!(dot.code, KeyCode::Char('.'));
    assert!(dot.alt && !dot.ctrl);

    let both = parse_binding("ctrl+alt+x").unwrap();
    assert!(both.ctrl && both.alt);
    assert_eq!(both.code, KeyCode::Char('x'));
}

#[test]
fn modifier_aliases_accepted() {
    assert!(parse_binding("control+a").unwrap().ctrl);
    assert!(parse_binding("meta+a").unwrap().alt);
    assert!(parse_binding("option+a").unwrap().alt);
}

#[test]
fn shift_modifier_is_ignored_in_spec() {
    // Shift is encoded in char case, so it is parsed but sets no flag.
    let b = parse_binding("shift+a").unwrap();
    assert!(!b.ctrl && !b.alt);
    assert_eq!(b.code, KeyCode::Char('a'));
}

#[test]
fn rejects_unknown_modifier_and_key() {
    assert!(parse_binding("hyper+a").is_err());
    assert!(parse_binding("nope").is_err());
}

#[test]
fn matches_requires_exact_modifiers() {
    let b = parse_binding("ctrl+c").unwrap();
    assert!(b.matches(&ev(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    // Missing the ctrl modifier should not match.
    assert!(!b.matches(&ev(KeyCode::Char('c'), KeyModifiers::empty())));
    // A different code should not match.
    assert!(!b.matches(&ev(KeyCode::Char('x'), KeyModifiers::CONTROL)));
}

#[test]
fn matches_ignores_shift_for_unmodified_binding() {
    // Pressing 'G' arrives as Char('G') + SHIFT; a "G" binding must match.
    let b = parse_binding("G").unwrap();
    assert!(b.matches(&ev(KeyCode::Char('G'), KeyModifiers::SHIFT)));
}

#[test]
fn unmodified_binding_rejects_ctrl_press() {
    let b = parse_binding("g").unwrap();
    assert!(!b.matches(&ev(KeyCode::Char('g'), KeyModifiers::CONTROL)));
}

#[test]
fn pressed_matches_any_in_list() {
    let binds = bind(&["Up", "k"]);
    assert!(pressed(&binds, &ev(KeyCode::Up, KeyModifiers::empty())));
    assert!(pressed(
        &binds,
        &ev(KeyCode::Char('k'), KeyModifiers::empty())
    ));
    assert!(!pressed(
        &binds,
        &ev(KeyCode::Char('j'), KeyModifiers::empty())
    ));
}

#[test]
fn malformed_local_config_reports_warning_and_falls_back() {
    let dir = std::env::temp_dir().join(format!(
        "mantis_cfg_bad_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    // `tree_width` expects an integer; a string makes parsing fail.
    fs::write(dir.join("mantis.toml"), "tree_width = \"oops\"\n").unwrap();

    let (_config, _path, error) = load(&dir);
    // The malformed file is ignored (the loader falls back to a valid
    // lower-precedence config or defaults) but the warning is still surfaced.
    let msg = error.expect("malformed config should produce a warning");
    assert!(
        msg.contains("mantis.toml"),
        "warning should name the file: {msg}"
    );

    fs::remove_dir_all(&dir).ok();
}

// ---- kitty keyboard protocol alternate-key matching -----------------------

#[cfg(unix)]
use crate::event_source::{AltKeys, CURRENT_ALT_KEYS};

/// Synthetic key event with explicit modifiers.
fn evm(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn set_alt(shifted: Option<char>, base: Option<char>) {
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys { shifted, base }));
}

fn reset_alt() {
    set_alt(None, None);
}

#[test]
#[cfg(unix)]
fn matches_uses_base_key_for_alphabetic_binding() {
    let binding = parse_binding("p").unwrap();

    // A Russian-layout key event: physical P key produces 'з' (U+0437).
    let event = evm(KeyCode::Char('з'), KeyModifiers::empty());

    // Without base key: no match.
    reset_alt();
    assert!(!binding.matches(&event));

    // With base key 'p': matches.
    set_alt(Some('З'), Some('p'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_uses_base_key_with_shift_for_capital_binding() {
    let binding = parse_binding("G").unwrap();

    // Russian Shift+G (physical 'y' on US → 'Н' in Russian).
    let event = evm(KeyCode::Char('Н'), KeyModifiers::SHIFT);

    // Base 'y' + Shift → 'Y' → does NOT match 'G'.
    set_alt(Some('Н'), Some('y'));
    assert!(!binding.matches(&event));

    // Base 'g' + Shift → 'G' → matches.
    set_alt(Some('Г'), Some('g'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_uses_shifted_key_for_symbol_binding() {
    let binding = parse_binding("?").unwrap();

    // US Shift+/ produces '?'. Kitty sends 47:63 (primary='/', shifted='?').
    let event = evm(KeyCode::Char('/'), KeyModifiers::SHIFT);

    // No base-layout key (2-field form), shifted = Some('?').
    set_alt(Some('?'), None);
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_uses_base_key_for_non_letter_symbol() {
    // On a Russian layout, the physical '/' key (US) produces '.'.
    // With the base field, `/` should still match the binding.
    let binding = parse_binding("/").unwrap();

    // Russian '.' key event with base='/'.
    let event = evm(KeyCode::Char('.'), KeyModifiers::empty());
    set_alt(Some('.'), Some('/'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_uses_base_key_with_us_shift_for_symbol() {
    // On a Russian layout, Shift+physical '/' (US) produces ','.
    // Base='/' + Shift should produce '?' via US shift mapping.
    let binding = parse_binding("?").unwrap();

    let event = evm(KeyCode::Char(','), KeyModifiers::SHIFT);
    set_alt(Some(','), Some('/'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_symbol_base_only_no_mismatch() {
    // base='/' with no shift should NOT match '?' binding.
    let binding = parse_binding("?").unwrap();

    let event = evm(KeyCode::Char('.'), KeyModifiers::empty());
    set_alt(Some('.'), Some('/'));
    assert!(!binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_does_not_affect_non_char_bindings() {
    let binding = parse_binding("Enter").unwrap();
    let event = evm(KeyCode::Enter, KeyModifiers::empty());

    // Even with stale alternate keys, a non-Char binding matches against key.code.
    set_alt(Some('З'), Some('p'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_does_not_substitute_for_wrong_modifiers() {
    let binding = parse_binding("p").unwrap();
    let event = evm(KeyCode::Char('з'), KeyModifiers::ALT);

    // Base key is 'p' but event has Alt modifier — binding requires no modifier.
    set_alt(Some('З'), Some('p'));
    assert!(!binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_falls_back_to_key_code_when_no_alternates() {
    let binding = parse_binding("g").unwrap();
    let event = evm(KeyCode::Char('g'), KeyModifiers::empty());

    reset_alt();
    assert!(binding.matches(&event));
}

#[test]
#[cfg(unix)]
fn pressed_honours_current_alt_keys() {
    let bindings = bind(&["ctrl+p", "ctrl+g"]);
    let event = evm(KeyCode::Char('з'), KeyModifiers::CONTROL);

    // Without base key: no match (з != p, з != g).
    reset_alt();
    assert!(!pressed(&bindings, &event));

    // With base key 'p': ctrl+p matches.
    set_alt(Some('З'), Some('p'));
    assert!(pressed(&bindings, &event));

    reset_alt();
}

// ---- end kitty-protocol tests --------------------------------------------

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mantis_cfg_{tag}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn init_writes_stub_user_config_and_default_reference() {
    let dir = scratch_dir("init");
    let user = dir.join("mantis.toml");
    init_config_dir(&user);

    // User config is a minimal stub, not the full template.
    let stub = fs::read_to_string(&user).unwrap();
    assert!(stub.contains("your overrides only"), "stub missing header");
    assert!(
        !stub.contains("Open config in editor"),
        "user config must not be the full template"
    );
    // The fully-commented reference is written separately and parses as Config.
    let reference = fs::read_to_string(dir.join("mantis.default.toml")).unwrap();
    assert!(reference.contains("Open config in editor"));
    let cfg: Config = toml::from_str(&reference).expect("default reference should parse");
    assert_eq!(cfg.tree.width, 28);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_never_overwrites_existing_user_config() {
    let dir = scratch_dir("noclobber");
    let user = dir.join("mantis.toml");
    fs::write(&user, "tree_width = 99\n").unwrap();
    init_config_dir(&user);
    // Upgrade path must leave the user's file byte-for-byte untouched.
    assert_eq!(fs::read_to_string(&user).unwrap(), "tree_width = 99\n");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn refresh_default_reference_rewrites_only_when_stale() {
    let dir = scratch_dir("refresh");
    // Missing -> written.
    assert!(refresh_default_reference(&dir));
    // Identical -> skipped.
    assert!(!refresh_default_reference(&dir));
    // Stale (simulating an old version) -> rewritten to the current template.
    fs::write(dir.join("mantis.default.toml"), "# outdated\n").unwrap();
    assert!(refresh_default_reference(&dir));
    assert_eq!(
        fs::read_to_string(dir.join("mantis.default.toml")).unwrap(),
        DEFAULT_CONFIG_TEMPLATE
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn sparse_toml_omits_defaults_and_keeps_overrides() {
    let mut cfg = Config::default();
    let out = sparse_toml(&cfg);
    // A pristine config serialises to (essentially) nothing.
    assert!(
        !out.contains("[tree]"),
        "default-valued key should be omitted: {out}"
    );

    cfg.tree.width = 42;
    cfg.tree.show_hidden = true;
    let out = sparse_toml(&cfg);
    assert!(out.contains("[tree]"), "missing [tree] section: {out}");
    assert!(out.contains("width = 42"), "override missing: {out}");
    assert!(
        out.contains("show_hidden = true"),
        "override missing: {out}"
    );
    // Untouched defaults stay out of the file.
    assert!(!out.contains("word_wrap"), "default leaked: {out}");

    // Round-trips: a sparse file re-parses to the same effective values.
    let reparsed: Config = toml::from_str(&out).unwrap();
    assert_eq!(reparsed.tree.width, 42);
    assert!(reparsed.tree.show_hidden);
    assert!(!reparsed.content.word_wrap); // falls back to default
}

#[test]
fn config_paths_are_local_first_then_global() {
    let root = Path::new("/a/b/c");
    let paths = config_paths(root);
    // Project-local: root first, then each ancestor.
    assert_eq!(paths[0], PathBuf::from("/a/b/c/mantis.toml"));
    assert_eq!(paths[1], PathBuf::from("/a/b/mantis.toml"));
    assert_eq!(paths[2], PathBuf::from("/a/mantis.toml"));
    assert_eq!(paths[3], PathBuf::from("/mantis.toml"));
    // Global config (if resolvable) comes after all local candidates.
    if let Some(global) = global_config_path() {
        assert_eq!(*paths.last().unwrap(), global);
        assert!(paths.iter().position(|p| *p == global).unwrap() >= 4);
    }
}

use super::validate::validate_keys;

#[test]
fn git_show_untracked_defaults_to_true() {
    let cfg = Config::default();
    assert!(cfg.git.show_untracked);
}

#[test]
fn git_show_untracked_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            show_untracked: false,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(!parsed.git.show_untracked);
}

#[test]
fn git_show_ignored_defaults_to_false() {
    let cfg = Config::default();
    assert!(!cfg.git.show_ignored);
}

#[test]
fn git_show_ignored_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            show_ignored: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(parsed.git.show_ignored);
}

#[test]
fn validate_keys_accepts_full_default_template() {
    // The shipped template must validate cleanly against the schema.
    assert!(validate_keys(DEFAULT_CONFIG_TEMPLATE).is_empty());
}

#[test]
fn icons_defaults_to_false() {
    let cfg = Config::default();
    assert!(!cfg.tree.icons);
}

#[test]
fn icons_round_trips_through_serde() {
    let cfg = Config {
        tree: TreeConfig {
            icons: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(parsed.tree.icons);
}

#[test]
fn icons_false_round_trips_through_serde() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(!parsed.tree.icons);
}

// -- find_files keybinding ---------------------------------------------------

#[test]
fn find_files_defaults_to_ctrl_f() {
    let cfg = Config::default();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "default find_files must include ctrl+f"
    );
}

#[test]
fn find_files_can_be_overridden_in_config() {
    let toml_str = r#"
[keys]
find_files = ["ctrl+x"]
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)) }),
        "overridden find_files must match ctrl+x"
    );
    assert!(
        !cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "overridden find_files must not include ctrl+f"
    );
}

#[test]
fn find_files_without_config_key_gets_default_from_serde_container() {
    let toml_str = r#"
[keys]
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "omitted find_files must default to ctrl+f via serde container default"
    );
}

#[test]
fn validate_keys_flags_unknown_top_level_key_with_suggestion() {
    let warnings = validate_keys("tre = 30\n");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("unknown key 'tre'") && warnings[0].contains("did you mean 'tree'?"),
        "expected nearest-match hint: {}",
        warnings[0]
    );
}

#[test]
fn validate_keys_reports_nested_unknown_keys_by_path() {
    let warnings = validate_keys("[keys]\nqiut = [\"q\"]\n\n[theme]\nacent = \"red\"\n");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("unknown key 'keys.qiut'") && w.contains("did you mean 'quit'?")),
        "missing keys.qiut warning: {warnings:?}"
    );
    assert!(
        warnings.iter().any(
            |w| w.contains("unknown key 'theme.acent'") && w.contains("did you mean 'accent'?")
        ),
        "missing theme.acent warning: {warnings:?}"
    );
}

#[test]
fn validate_keys_omits_hint_when_nothing_close() {
    let warnings = validate_keys("completely_unrelated_setting = true\n");
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("unknown key 'completely_unrelated_setting'"));
    assert!(
        !warnings[0].contains("did you mean"),
        "should not guess for a distant key: {}",
        warnings[0]
    );
}

#[test]
fn diff_mode_defaults_to_all() {
    let cfg = Config::default();
    assert_eq!(cfg.git.diff.mode, crate::app::DiffMode::All);
}

#[test]
fn diff_mode_staged_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            diff: GitDiffConfig {
                mode: crate::app::DiffMode::Staged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.git.diff.mode, crate::app::DiffMode::Staged);
}

#[test]
fn diff_mode_unstaged_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            diff: GitDiffConfig {
                mode: crate::app::DiffMode::Unstaged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.git.diff.mode, crate::app::DiffMode::Unstaged);
}

#[test]
fn deprecated_diff_mode_is_silently_folded_to_default() {
    let dir = std::env::temp_dir().join(format!(
        "mantis_cfg_diff_mode_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    // Old flat diff_mode key; "stagged" is invalid, so migration falls back to All.
    fs::write(dir.join("mantis.toml"), "diff_mode = \"stagged\"\n").unwrap();

    let (config, path, error) = load(&dir);
    assert!(path.is_some());
    // After migrate_legacy_git_fields, the invalid value is silently replaced
    // by the default (All). No warning because diff_mode is a known deprecated key.
    assert_eq!(config.git.diff.mode, crate::app::DiffMode::All);
    assert!(
        error.is_none(),
        "deprecated key should not produce a warning: {:?}",
        error
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_key_surfaces_as_warning_but_config_still_loads() {
    let dir = std::env::temp_dir().join(format!(
        "mantis_cfg_unknown_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    // Valid TOML, valid value, but a typo'd key name.
    fs::write(dir.join("mantis.toml"), "tree_widht = 40\n").unwrap();

    let (config, path, error) = load(&dir);
    // The config still loads (the typo'd key is simply ignored)...
    assert!(path.is_some());
    assert_eq!(config.tree.width, Config::default().tree.width);
    // ...but the typo is surfaced with a suggestion.
    let msg = error.expect("unknown key should produce a warning");
    assert!(
        msg.contains("tree_widht"),
        "warning should name the bad key: {msg}"
    );

    fs::remove_dir_all(&dir).ok();
}

// Serialize env-var manipulation across parallel tests.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn unique_migrate_tmp(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mantis_migrate_{}_{}_{label}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    ))
}

#[test]
#[cfg(not(windows))]
fn migrate_renames_old_dir_to_new() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = unique_migrate_tmp("a");
    let old_dir = tmp.join("tree-viewer");
    let new_dir = tmp.join("mantis");
    fs::create_dir_all(&old_dir).unwrap();
    fs::write(old_dir.join("tv.toml"), b"# config").unwrap();
    fs::write(old_dir.join("tv.default.toml"), b"# default").unwrap();

    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    migrate_legacy_config();
    std::env::remove_var("XDG_CONFIG_HOME");

    assert!(new_dir.exists(), "new dir created");
    assert!(!old_dir.exists(), "old dir renamed away");
    assert!(new_dir.join("mantis.toml").exists(), "tv.toml renamed");
    assert!(
        new_dir.join("mantis.default.toml").exists(),
        "tv.default.toml renamed"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
#[cfg(not(windows))]
fn migrate_skips_when_new_dir_exists() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = unique_migrate_tmp("b");
    let old_dir = tmp.join("tree-viewer");
    let new_dir = tmp.join("mantis");
    fs::create_dir_all(&old_dir).unwrap();
    fs::write(old_dir.join("tv.toml"), b"# config").unwrap();
    fs::create_dir_all(&new_dir).unwrap();

    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    migrate_legacy_config();
    std::env::remove_var("XDG_CONFIG_HOME");

    assert!(
        old_dir.exists(),
        "old dir untouched when new dir already exists"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
#[cfg(not(windows))]
fn migrate_no_op_when_old_dir_absent() {
    let _guard = ENV_LOCK.lock().unwrap();
    let tmp = unique_migrate_tmp("c");
    fs::create_dir_all(&tmp).unwrap();

    std::env::set_var("XDG_CONFIG_HOME", &tmp);
    migrate_legacy_config();
    std::env::remove_var("XDG_CONFIG_HOME");

    assert!(
        !tmp.join("mantis").exists(),
        "nothing created when old dir absent"
    );
    fs::remove_dir_all(&tmp).ok();
}

#[test]
fn legacy_flat_git_keys_fold_into_git_config() {
    let toml_str = r#"
git_status = false
ignore_gitignore = true
git_show_deleted = true
git_show_untracked = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_git_fields();
    assert!(!cfg.git.status, "legacy git_status=false should fold");
    assert!(
        cfg.git.ignore_gitignore,
        "legacy ignore_gitignore=true should fold"
    );
    assert!(
        cfg.git.show_deleted,
        "legacy git_show_deleted=true should fold"
    );
    assert!(
        !cfg.git.show_untracked,
        "legacy git_show_untracked=false should fold"
    );
}

#[test]
fn new_git_table_populates_fields() {
    let toml_str = r#"
[git]
status = false
ignore_gitignore = true
show_deleted = true
show_untracked = false

[git.diff]
mode = "staged"
side_by_side = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(!cfg.git.status);
    assert!(cfg.git.ignore_gitignore);
    assert!(cfg.git.show_deleted);
    assert!(!cfg.git.show_untracked);
    assert_eq!(cfg.git.diff.mode, crate::app::DiffMode::Staged);
    assert!(cfg.git.diff.side_by_side);
}

#[test]
fn legacy_key_wins_when_both_present() {
    // When both new [git] and legacy flat key exist, migrate folds last so
    // legacy overwrites. Legacy key must come before [git] header in TOML
    // to be at the top level (otherwise it's inside [git]).
    let toml_str = r#"
git_status = true

[git]
status = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_git_fields();
    // Legacy runs last, so git_status=true overwrites the [git] status=false.
    assert!(
        cfg.git.status,
        "legacy git_status should win over [git] status"
    );
}

#[test]
fn legacy_keys_produce_no_validate_warnings() {
    let warnings =
        validate_keys("git_status = false\nignore_gitignore = true\ndiff_mode = \"all\"\n");
    assert!(
        warnings.is_empty(),
        "deprecated keys should not produce warnings: {:?}",
        warnings
    );
}

#[test]
fn save_returns_err_on_unwritable_dir() {
    let dir = scratch_dir("save_err");
    // Don't create the subdir — save to a non-existent path.
    let bad = dir.join("nonexistent").join("mantis.toml");
    let cfg = Config::default();
    let result = save(&cfg, &bad);
    assert!(result.is_err(), "save to unwritable dir should fail");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn save_returns_ok_on_success_and_round_trips() {
    let dir = scratch_dir("save_ok");
    let path = dir.join("mantis.toml");
    let cfg = Config {
        tree: TreeConfig {
            width: 42,
            show_hidden: true,
            ..Default::default()
        },
        ..Config::default()
    };

    let result = save(&cfg, &path);
    assert!(result.is_ok(), "save should succeed: {:?}", result);

    // Round-trip: re-load and verify overrides survive, defaults remain.
    let loaded_raw = fs::read_to_string(&path).unwrap();
    let loaded: Config = toml::from_str(&loaded_raw).unwrap();
    assert_eq!(loaded.tree.width, 42);
    assert!(loaded.tree.show_hidden);
    assert!(!loaded.content.word_wrap); // falls back to default

    // Output is sparse — only non-default keys appear.
    assert!(loaded_raw.contains("width = 42"), "{loaded_raw}");
    assert!(
        !loaded_raw.contains("word_wrap"),
        "default leaked: {loaded_raw}"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn unknown_key_still_warns_even_alongside_deprecated_keys() {
    let warnings = validate_keys("git_staus = true\n");
    assert!(
        warnings.iter().any(|w| w.contains("git_staus")),
        "typo on deprecated key should still warn: {:?}",
        warnings
    );
}

#[test]
fn diff_mode_serde_round_trips() {
    #[derive(Serialize, Deserialize)]
    struct Wrap {
        m: crate::app::DiffMode,
    }
    let cases = [
        (crate::app::DiffMode::All, "m = \"all\""),
        (crate::app::DiffMode::Staged, "m = \"staged\""),
        (crate::app::DiffMode::Unstaged, "m = \"unstaged\""),
    ];
    for (mode, expected_str) in &cases {
        let toml_str = toml::to_string_pretty(&Wrap { m: *mode }).unwrap();
        assert!(toml_str.contains(expected_str), "got: {toml_str:?}");
        let wrap: Wrap = toml::from_str(expected_str).unwrap();
        assert_eq!(wrap.m, *mode);
    }
}

#[test]
fn sparse_toml_emits_nested_git_form_not_flat_keys() {
    let mut cfg = Config::default();
    cfg.git.status = false;
    cfg.git.show_untracked = false;
    let out = sparse_toml(&cfg);
    // Should NOT contain the legacy flat key names.
    assert!(!out.contains("git_status"), "flat key leaked: {out}");
    // Should contain the new nested form.
    assert!(out.contains("[git]"), "missing [git] section: {out}");
    assert!(
        out.contains("status = false"),
        "missing status override: {out}"
    );
    // Round-trips: re-parsing yields the same effective values.
    let reparsed: Config = toml::from_str(&out).unwrap();
    assert!(!reparsed.git.status);
    assert!(!reparsed.git.show_untracked);
    // Defaults not in the output still resolve correctly.
    assert!(reparsed.git.show_deleted == Config::default().git.show_deleted);
}

// ---- tree/content/search config grouping -----------------------------------

#[test]
fn legacy_flat_tree_keys_fold_into_tree_config() {
    let toml_str = r#"
show_hidden = true
tree_width = 40
tree_independent_scroll = true
indent_guides = false
icons = true
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(cfg.tree.show_hidden, "legacy show_hidden=true should fold");
    assert_eq!(cfg.tree.width, 40, "legacy tree_width=40 should fold");
    assert!(
        cfg.tree.independent_scroll,
        "legacy tree_independent_scroll=true should fold"
    );
    assert!(
        !cfg.tree.indent_guides,
        "legacy indent_guides=false should fold"
    );
    assert!(cfg.tree.icons, "legacy icons=true should fold");
}

#[test]
fn legacy_flat_content_keys_fold_into_content_config() {
    let toml_str = r#"
word_wrap = true
line_numbers = false
scrollbar = false
scroll_percentage = false
watch = true
show_file_info = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(cfg.content.word_wrap, "legacy word_wrap=true should fold");
    assert!(
        !cfg.content.line_numbers,
        "legacy line_numbers=false should fold"
    );
    assert!(!cfg.content.scrollbar, "legacy scrollbar=false should fold");
    assert!(
        !cfg.content.scroll_percentage,
        "legacy scroll_percentage=false should fold"
    );
    assert!(cfg.content.watch, "legacy watch=true should fold");
    assert!(
        !cfg.content.show_file_info,
        "legacy show_file_info=false should fold"
    );
}

#[test]
fn legacy_flat_search_keys_fold_into_search_config() {
    let toml_str = r#"
in_file_search = false
search_context_lines = 3
keep_search_query = true
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(
        !cfg.search.in_file_search,
        "legacy in_file_search=false should fold"
    );
    assert_eq!(
        cfg.search.context_lines, 3,
        "legacy search_context_lines=3 should fold"
    );
    assert!(
        cfg.search.keep_query,
        "legacy keep_search_query=true should fold"
    );
}

#[test]
fn new_tree_table_populates_fields() {
    let toml_str = r#"
[tree]
show_hidden = true
width = 40
independent_scroll = true
indent_guides = false
icons = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.tree.show_hidden);
    assert_eq!(cfg.tree.width, 40);
    assert!(cfg.tree.independent_scroll);
    assert!(!cfg.tree.indent_guides);
    assert!(cfg.tree.icons);
}

#[test]
fn new_content_table_populates_fields() {
    let toml_str = r#"
[content]
word_wrap = true
line_numbers = false
scrollbar = false
scroll_percentage = false
watch = true
show_file_info = false
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.content.word_wrap);
    assert!(!cfg.content.line_numbers);
    assert!(!cfg.content.scrollbar);
    assert!(!cfg.content.scroll_percentage);
    assert!(cfg.content.watch);
    assert!(!cfg.content.show_file_info);
}

#[test]
fn new_search_table_populates_fields() {
    let toml_str = r#"
[search]
in_file_search = false
context_lines = 3
keep_query = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(!cfg.search.in_file_search);
    assert_eq!(cfg.search.context_lines, 3);
    assert!(cfg.search.keep_query);
}

#[test]
fn legacy_flat_keys_produce_no_validate_warnings() {
    let warnings = validate_keys(
        "show_hidden = true\ntree_width = 40\nword_wrap = true\nsearch_context_lines = 3\n",
    );
    assert!(
        warnings.is_empty(),
        "deprecated keys should not produce warnings: {:?}",
        warnings
    );
}

#[test]
fn typo_on_moved_key_still_warns() {
    let warnings = validate_keys("scrol_percentage = true\n");
    assert!(
        warnings.iter().any(|w| w.contains("scrol_percentage")),
        "typo on moved key should still warn: {:?}",
        warnings
    );
}

#[test]
fn recent_files_count_stays_top_level() {
    let toml_str = "recent_files_count = 25\n";
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.recent_files_count, 25);
}

#[test]
fn sparse_toml_emits_nested_tree_content_search_form() {
    let mut cfg = Config::default();
    cfg.tree.width = 50;
    cfg.content.word_wrap = true;
    cfg.search.context_lines = 5;
    let out = sparse_toml(&cfg);
    // Should contain the new nested sections, not flat keys.
    assert!(out.contains("[tree]"), "missing [tree] section: {out}");
    assert!(
        out.contains("[content]"),
        "missing [content] section: {out}"
    );
    assert!(out.contains("[search]"), "missing [search] section: {out}");
    assert!(out.contains("width = 50"), "missing width override: {out}");
    assert!(
        out.contains("word_wrap = true"),
        "missing word_wrap override: {out}"
    );
    assert!(
        out.contains("context_lines = 5"),
        "missing context_lines override: {out}"
    );
    // Flat legacy keys must not appear.
    assert!(!out.contains("tree_width"), "flat key leaked: {out}");
    assert!(
        !out.contains("search_context_lines"),
        "flat key leaked: {out}"
    );
    // Round-trips.
    let reparsed: Config = toml::from_str(&out).unwrap();
    assert_eq!(reparsed.tree.width, 50);
    assert!(reparsed.content.word_wrap);
    assert_eq!(reparsed.search.context_lines, 5);
}

#[test]
fn defaults_for_new_tables() {
    let cfg = Config::default();
    assert!(!cfg.tree.show_hidden);
    assert_eq!(cfg.tree.width, 28);
    assert!(!cfg.tree.independent_scroll);
    assert!(cfg.tree.indent_guides);
    assert!(!cfg.tree.icons);

    assert!(!cfg.content.word_wrap);
    assert!(cfg.content.line_numbers);
    assert!(cfg.content.scrollbar);
    assert!(cfg.content.scroll_percentage);
    assert!(!cfg.content.watch);
    assert!(cfg.content.show_file_info);

    assert!(cfg.search.in_file_search);
    assert_eq!(cfg.search.context_lines, 0);
    assert!(!cfg.search.keep_query);

    assert_eq!(cfg.recent_files_count, 10);
}
