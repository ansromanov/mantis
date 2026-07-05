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
