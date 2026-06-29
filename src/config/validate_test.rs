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
