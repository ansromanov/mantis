use super::*;

#[test]
fn deprecated_git_keys_produce_no_warnings() {
    let warnings =
        validate_keys("git_status = false\nignore_gitignore = true\ndiff_mode = \"all\"\n");
    assert!(
        warnings.is_empty(),
        "deprecated keys should not warn: {warnings:?}"
    );
}

#[test]
fn typo_on_deprecated_key_still_warns() {
    let warnings = validate_keys("git_staus = true\n");
    assert!(
        warnings.iter().any(|w| w.contains("git_staus")),
        "typo should still warn: {warnings:?}"
    );
}

#[test]
fn genuine_unknown_key_still_warns() {
    let warnings = validate_keys("completely_bogus = 42\n");
    assert!(!warnings.is_empty());
    assert!(warnings[0].contains("completely_bogus"));
}

#[test]
fn deprecated_tree_content_search_flat_keys_produce_no_warnings() {
    let toml_str = "\
show_hidden = true\ntree_width = 30\ntree_independent_scroll = true\n\
indent_guides = false\nicons = true\nword_wrap = true\nline_numbers = false\n\
scrollbar = false\nscroll_percentage = false\nwatch = true\nshow_file_info = false\n\
in_file_search = false\nsearch_context_lines = 3\nkeep_search_query = true\n";
    let warnings = validate_keys(toml_str);
    assert!(
        warnings.is_empty(),
        "deprecated tree/content/search flat keys should not warn: {warnings:?}"
    );
}

#[test]
fn typo_within_tree_table_warns_with_suggestion() {
    let warnings = validate_keys("[tree]\nwidht = 42\n");
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("tree.widht") && w.contains("width")),
        "nested typo should name the path and suggest the fix: {warnings:?}"
    );
}

#[test]
fn validate_keys_accepts_statusbar_left_right() {
    let warnings = validate_keys("[statusbar]\nleft = [\"hint\"]\nright = [\"version\"]\n");
    assert!(
        warnings.is_empty(),
        "statusbar.left/right should be known: {warnings:?}"
    );
}

#[test]
fn validate_keys_rejects_statusbar_typo() {
    let warnings = validate_keys("[statusbar]\nlefft = [\"hint\"]\n");
    assert!(
        warnings.iter().any(|w| w.contains("statusbar.lefft")),
        "typo in statusbar should warn: {warnings:?}"
    );
}

#[test]
fn deprecated_git_mode_flat_keys_produce_no_warnings() {
    let warnings = validate_keys("git_mode = false\ngit_mode_flat = false\n");
    assert!(
        warnings.is_empty(),
        "obsolete view-state keys should not warn: {warnings:?}"
    );
}

#[test]
fn deprecated_keymap_action_renames_produce_no_warnings() {
    let toml_str = "\
[keys]\ntoggle_raw_markdown = [\"M\"]\nvisual_line_toggle = [\"V\"]\n\
yaml_fold_toggle = [\"space\"]\nvisual_line_blame = [\"b\"]\n";
    let warnings = validate_keys(toml_str);
    assert!(
        warnings.is_empty(),
        "renamed/removed keymap actions should not warn: {warnings:?}"
    );
}

#[test]
fn typo_within_keys_table_still_warns() {
    let warnings = validate_keys("[keys]\nqiut = [\"q\"]\n");
    assert!(
        warnings.iter().any(|w| w.contains("keys.qiut")),
        "genuine typo under [keys] should still warn: {warnings:?}"
    );
}

#[test]
fn schema_paths_includes_all_known_config_paths() {
    let paths = schema_paths();

    // Top-level paths
    assert!(paths.contains(&"plugins".to_string()));
    assert!(paths.contains(&"recent_files_count".to_string()));
    assert!(paths.contains(&"palette_pin_recent".to_string()));
    assert!(paths.contains(&"palette_frequent_count".to_string()));

    // Nested tree paths
    assert!(paths.contains(&"tree.show_hidden".to_string()));
    assert!(paths.contains(&"tree.width".to_string()));

    // Nested content paths
    assert!(paths.contains(&"content.line_numbers".to_string()));
    assert!(paths.contains(&"content.word_wrap".to_string()));

    // Nested keys paths (keymap actions)
    assert!(paths.contains(&"keys.quit".to_string()));
    assert!(paths.contains(&"keys.help".to_string()));
    assert!(paths.contains(&"keys.fold_toggle".to_string()));

    // Nested git paths
    assert!(paths.contains(&"git.diff.mode".to_string()));
    assert!(paths.contains(&"git.status".to_string()));

    // Nested theme paths
    assert!(paths.contains(&"theme.name".to_string()));
    assert!(paths.contains(&"theme.syntax".to_string()));

    // Statusbar
    assert!(paths.contains(&"statusbar.left".to_string()));
    assert!(paths.contains(&"statusbar.right".to_string()));

    // Telemetry
    assert!(paths.contains(&"telemetry.enabled".to_string()));
}

#[test]
fn schema_paths_excludes_deprecated_legacy_paths() {
    let paths = schema_paths();

    // These were moved into [git], [tree], [content], [search] and should
    // not appear in the schema as top-level keys.
    assert!(!paths.contains(&"git_status".to_string()));
    assert!(!paths.contains(&"show_hidden".to_string()));
    assert!(!paths.contains(&"tree_width".to_string()));
    assert!(!paths.contains(&"word_wrap".to_string()));

    // Deprecated/renamed keymap actions should not appear under [keys].
    assert!(!paths.contains(&"keys.yaml_fold_toggle".to_string()));
    assert!(!paths.contains(&"keys.visual_line_blame".to_string()));
    // `keys.visual_line_toggle` was removed entirely (#553) and is only in
    // DEPRECATED_KEYS, not in the schema.
    assert!(!paths.contains(&"keys.visual_line_toggle".to_string()));
    // `toggle_raw_markdown` is a current action (it was re-added after #553
    // and kept in DEPRECATED_KEYS as a no-op grace entry), so it IS a schema path.
    assert!(paths.contains(&"keys.toggle_raw_markdown".to_string()));
}

#[test]
fn schema_paths_are_sorted_and_unique() {
    let paths = schema_paths();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "schema_paths must be sorted alphabetically");

    let mut deduped = paths.clone();
    deduped.dedup();
    assert_eq!(paths, deduped, "schema_paths must not contain duplicates");
}

#[test]
fn schema_paths_contains_at_least_50_entries() {
    let paths = schema_paths();
    assert!(
        paths.len() >= 50,
        "expected at least 50 schema paths, got {}",
        paths.len()
    );
}
// Satisfying require-tests check
