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
        "tv_cfg_bad_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    // `tree_width` expects an integer; a string makes parsing fail.
    fs::write(dir.join("tv.toml"), "tree_width = \"oops\"\n").unwrap();

    let (_config, _path, error) = load(&dir);
    // The malformed file is ignored (the loader falls back to a valid
    // lower-precedence config or defaults) but the warning is still surfaced.
    let msg = error.expect("malformed config should produce a warning");
    assert!(
        msg.contains("tv.toml"),
        "warning should name the file: {msg}"
    );

    fs::remove_dir_all(&dir).ok();
}

fn scratch_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "tv_cfg_{tag}_{}_{}",
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
    let user = dir.join("tv.toml");
    init_config_dir(&user);

    // User config is a minimal stub, not the full template.
    let stub = fs::read_to_string(&user).unwrap();
    assert!(stub.contains("your overrides only"), "stub missing header");
    assert!(
        !stub.contains("Open config in editor"),
        "user config must not be the full template"
    );
    // The fully-commented reference is written separately and parses as Config.
    let reference = fs::read_to_string(dir.join("tv.default.toml")).unwrap();
    assert!(reference.contains("Open config in editor"));
    let cfg: Config = toml::from_str(&reference).expect("default reference should parse");
    assert_eq!(cfg.tree_width, 28);

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn init_never_overwrites_existing_user_config() {
    let dir = scratch_dir("noclobber");
    let user = dir.join("tv.toml");
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
    fs::write(dir.join("tv.default.toml"), "# outdated\n").unwrap();
    assert!(refresh_default_reference(&dir));
    assert_eq!(
        fs::read_to_string(dir.join("tv.default.toml")).unwrap(),
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
        !out.contains("tree_width"),
        "default-valued key should be omitted: {out}"
    );

    cfg.tree_width = 42;
    cfg.show_hidden = true;
    let out = sparse_toml(&cfg);
    assert!(out.contains("tree_width = 42"), "override missing: {out}");
    assert!(
        out.contains("show_hidden = true"),
        "override missing: {out}"
    );
    // Untouched defaults stay out of the file.
    assert!(!out.contains("word_wrap"), "default leaked: {out}");

    // Round-trips: a sparse file re-parses to the same effective values.
    let reparsed: Config = toml::from_str(&out).unwrap();
    assert_eq!(reparsed.tree_width, 42);
    assert!(reparsed.show_hidden);
    assert!(!reparsed.word_wrap); // falls back to default
}

#[test]
fn config_paths_are_local_first_then_global() {
    let root = Path::new("/a/b/c");
    let paths = config_paths(root);
    // Project-local: root first, then each ancestor.
    assert_eq!(paths[0], PathBuf::from("/a/b/c/tv.toml"));
    assert_eq!(paths[1], PathBuf::from("/a/b/tv.toml"));
    assert_eq!(paths[2], PathBuf::from("/a/tv.toml"));
    assert_eq!(paths[3], PathBuf::from("/tv.toml"));
    // Global config (if resolvable) comes after all local candidates.
    if let Some(global) = global_config_path() {
        assert_eq!(*paths.last().unwrap(), global);
        assert!(paths.iter().position(|p| *p == global).unwrap() >= 4);
    }
}

use super::validate::validate_keys;

#[test]
fn validate_keys_accepts_full_default_template() {
    // The shipped template must validate cleanly against the schema.
    assert!(validate_keys(DEFAULT_CONFIG_TEMPLATE).is_empty());
}

#[test]
fn icons_defaults_to_false() {
    let cfg = Config::default();
    assert!(!cfg.icons);
}

#[test]
fn icons_round_trips_through_serde() {
    let cfg = Config {
        icons: true,
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(parsed.icons);
}

#[test]
fn icons_false_round_trips_through_serde() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(!parsed.icons);
}

#[test]
fn validate_keys_flags_unknown_top_level_key_with_suggestion() {
    let warnings = validate_keys("tre_width = 30\n");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains("unknown key 'tre_width'")
            && warnings[0].contains("did you mean 'tree_width'?"),
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
fn unknown_key_surfaces_as_warning_but_config_still_loads() {
    let dir = std::env::temp_dir().join(format!(
        "tv_cfg_unknown_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).unwrap();
    // Valid TOML, valid value, but a typo'd key name.
    fs::write(dir.join("tv.toml"), "tree_widht = 40\n").unwrap();

    let (config, path, error) = load(&dir);
    // The config still loads (the typo'd key is simply ignored)...
    assert!(path.is_some());
    assert_eq!(config.tree_width, Config::default().tree_width);
    // ...but the typo is surfaced with a suggestion.
    let msg = error.expect("unknown key should produce a warning");
    assert!(
        msg.contains("tree_widht") && msg.contains("tree_width"),
        "warning should name the bad key and suggestion: {msg}"
    );

    fs::remove_dir_all(&dir).ok();
}
