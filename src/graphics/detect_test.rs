use super::*;

#[test]
fn kitty_window_id_alone_is_enough() {
    assert!(env_hint_from(true, false, None, None));
}

#[test]
fn wezterm_executable_alone_is_enough() {
    assert!(env_hint_from(false, true, None, None));
}

#[test]
fn term_names_are_matched_as_substrings() {
    assert!(env_hint_from(false, false, Some("xterm-kitty"), None));
    assert!(env_hint_from(false, false, Some("xterm-ghostty"), None));
}

#[test]
fn term_program_is_matched_case_insensitively() {
    assert!(env_hint_from(false, false, None, Some("ghostty")));
    assert!(env_hint_from(false, false, None, Some("WezTerm")));
}

#[test]
fn plain_xterm_is_not_a_match() {
    assert!(!env_hint_from(
        false,
        false,
        Some("xterm-256color"),
        Some("Apple_Terminal")
    ));
    assert!(!env_hint_from(false, false, None, None));
}

#[test]
fn support_is_false_until_detection_runs() {
    // `detect` is never called in the test binary, so the cached value stays at
    // its `unwrap_or(false)` default and image preview stays off.
    assert!(!kitty_graphics_supported());
}
