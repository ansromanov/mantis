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
