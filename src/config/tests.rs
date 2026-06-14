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
        "tv_cfg_bad_{}_{:?}",
        std::process::id(),
        std::time::SystemTime::now()
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

#[test]
fn install_default_writes_template_and_is_parseable() {
    let dir = std::env::temp_dir().join(format!(
        "tv_cfg_install_{}_{:?}",
        std::process::id(),
        std::time::SystemTime::now()
    ));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tv.toml");
    install_default(&path);
    let content = fs::read_to_string(&path).unwrap();
    // Template must include the config hint comment.
    assert!(
        content.contains("Open config in editor"),
        "template missing palette hint"
    );
    // Template must be valid TOML that parses as Config with defaults.
    let cfg: Config = toml::from_str(&content).expect("default template should parse");
    assert!(!cfg.show_hidden);
    assert_eq!(cfg.tree_width, 28);
    fs::remove_dir_all(&dir).ok();
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

#[test]
fn validate_keys_accepts_full_default_template() {
    // The shipped template must validate cleanly against the schema.
    assert!(validate_keys(DEFAULT_CONFIG_TEMPLATE).is_empty());
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
        "tv_cfg_unknown_{}_{:?}",
        std::process::id(),
        std::time::SystemTime::now()
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
