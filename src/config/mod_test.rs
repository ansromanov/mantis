use super::*;

use std::path::PathBuf;

use super::validate::validate_keys;

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

#[test]
fn validate_keys_accepts_full_default_template() {
    // The shipped template must validate cleanly against the schema.
    assert!(validate_keys(DEFAULT_CONFIG_TEMPLATE).is_empty());
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
fn init_writes_static_keys_reference() {
    let dir = scratch_dir("static_keys");
    let user = dir.join("mantis.toml");
    init_config_dir(&user);

    // Static keys reference is written alongside user config.
    let static_keys = fs::read_to_string(dir.join("mantis.static.toml"))
        .expect("mantis.static.toml should be created");
    assert!(
        static_keys.contains("Reserved modal keybindings"),
        "missing header in static keys"
    );
    assert!(
        static_keys.contains("modal_keys"),
        "missing [modal_keys] section"
    );
    assert!(
        static_keys.contains("close"),
        "missing close binding documentation"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn refresh_static_keys_reference_rewrites_only_when_stale() {
    let dir = scratch_dir("static_keys_refresh");
    // Missing -> written.
    assert!(refresh_static_keys_reference(&dir));
    // Identical -> skipped.
    assert!(!refresh_static_keys_reference(&dir));
    // Stale (simulating an old version) -> rewritten to the current template.
    fs::write(dir.join("mantis.static.toml"), "# outdated\n").unwrap();
    assert!(refresh_static_keys_reference(&dir));

    fs::remove_dir_all(&dir).ok();
}
