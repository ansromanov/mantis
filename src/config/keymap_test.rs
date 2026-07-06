use super::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[cfg(unix)]
use crate::event_source::AltKeys;

fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

#[cfg(unix)]
fn set_alt(shifted: Option<char>, base: Option<char>) {
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys { shifted, base }));
}

#[cfg(unix)]
fn reset_alt() {
    set_alt(None, None);
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

// ---- kitty keyboard protocol alternate-key matching -----------------------

#[test]
#[cfg(unix)]
fn matches_uses_base_key_for_alphabetic_binding() {
    let binding = parse_binding("p").unwrap();

    // A Russian-layout key event: physical P key produces 'з' (U+0437).
    let event = ev(KeyCode::Char('з'), KeyModifiers::empty());

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
    let event = ev(KeyCode::Char('Н'), KeyModifiers::SHIFT);

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
    let event = ev(KeyCode::Char('/'), KeyModifiers::SHIFT);

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
    let event = ev(KeyCode::Char('.'), KeyModifiers::empty());
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

    let event = ev(KeyCode::Char(','), KeyModifiers::SHIFT);
    set_alt(Some(','), Some('/'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_symbol_base_only_no_mismatch() {
    // base='/' with no shift should NOT match '?' binding.
    let binding = parse_binding("?").unwrap();

    let event = ev(KeyCode::Char('.'), KeyModifiers::empty());
    set_alt(Some('.'), Some('/'));
    assert!(!binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_does_not_affect_non_char_bindings() {
    let binding = parse_binding("Enter").unwrap();
    let event = ev(KeyCode::Enter, KeyModifiers::empty());

    // Even with stale alternate keys, a non-Char binding matches against key.code.
    set_alt(Some('З'), Some('p'));
    assert!(binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_does_not_substitute_for_wrong_modifiers() {
    let binding = parse_binding("p").unwrap();
    let event = ev(KeyCode::Char('з'), KeyModifiers::ALT);

    // Base key is 'p' but event has Alt modifier — binding requires no modifier.
    set_alt(Some('З'), Some('p'));
    assert!(!binding.matches(&event));

    reset_alt();
}

#[test]
#[cfg(unix)]
fn matches_alt_keys_falls_back_to_key_code_when_no_alternates() {
    let binding = parse_binding("g").unwrap();
    let event = ev(KeyCode::Char('g'), KeyModifiers::empty());

    reset_alt();
    assert!(binding.matches(&event));
}

#[test]
#[cfg(unix)]
fn pressed_honours_current_alt_keys() {
    let bindings = bind(&["ctrl+p", "ctrl+g"]);
    let event = ev(KeyCode::Char('з'), KeyModifiers::CONTROL);

    // Without base key: no match (з != p, з != g).
    reset_alt();
    assert!(!pressed(&bindings, &event));

    // With base key 'p': ctrl+p matches.
    set_alt(Some('З'), Some('p'));
    assert!(pressed(&bindings, &event));

    reset_alt();
}

// ---- legacy terminal fallback (no keyboard enhancement) --------------------
//
// On legacy terminals (macOS Terminal.app, plain xterm, SSH) without the kitty
// keyboard protocol, Ctrl+Letter and Ctrl+Shift+Letter produce identical events:
// both send the lowercase char with the CONTROL modifier. `KeyBinding::matches`
// must accept either case for Ctrl+letter bindings when no alt-keys are reported
// — otherwise the shift-variant binding (e.g. ctrl+G for git_mode_toggle) can
// never fire.

#[test]
#[cfg(unix)]
fn legacy_terminal_ctrl_uppercase_matches_lowercase_event() {
    // ctrl+G (uppercase = Ctrl+Shift+G on enhanced terminals) must match
    // a lowercase 'g' with CONTROL on legacy terminals.
    let binding = parse_binding("ctrl+G").unwrap();
    let event = ev(KeyCode::Char('g'), KeyModifiers::CONTROL);

    reset_alt();
    assert!(
        binding.matches(&event),
        "ctrl+G must match Char('g')+CONTROL on a legacy terminal"
    );
}

#[test]
#[cfg(unix)]
fn legacy_terminal_ctrl_lowercase_still_matches_lowercase() {
    // ctrl+g (lowercase) must still match normally on legacy terminals.
    let binding = parse_binding("ctrl+g").unwrap();
    let event = ev(KeyCode::Char('g'), KeyModifiers::CONTROL);

    reset_alt();
    assert!(binding.matches(&event));
}

#[test]
#[cfg(unix)]
fn legacy_terminal_ctrl_binding_rejects_different_char() {
    // ctrl+g must NOT match a different letter on legacy terminals.
    let binding = parse_binding("ctrl+g").unwrap();
    let event = ev(KeyCode::Char('x'), KeyModifiers::CONTROL);

    reset_alt();
    assert!(!binding.matches(&event));
}

#[test]
#[cfg(unix)]
fn legacy_terminal_unmodified_binding_not_affected() {
    // Non-ctrl bindings (e.g. 'G' for content_bottom) must NOT become
    // case-insensitive on legacy terminals. 'G' should only match 'G'.
    let binding = parse_binding("G").unwrap();
    let event = ev(KeyCode::Char('g'), KeyModifiers::empty());

    reset_alt();
    assert!(
        !binding.matches(&event),
        "unmodified 'G' must NOT match lowercase 'g' on a legacy terminal"
    );
}

#[test]
#[cfg(unix)]
fn legacy_terminal_ctrl_uppercase_does_not_match_when_alt_keys_present() {
    // On an enhanced terminal with alt-keys, ctrl+G must NOT match
    // a lowercase 'g' — they are distinct events when the protocol works.
    let binding = parse_binding("ctrl+G").unwrap();
    let event = ev(KeyCode::Char('g'), KeyModifiers::CONTROL);

    set_alt(Some('G'), Some('g'));
    assert!(
        !binding.matches(&event),
        "ctrl+G must NOT match Char('g') when alt-keys are available"
    );
    reset_alt();
}

#[test]
#[cfg(unix)]
fn legacy_terminal_lowercase_ctrl_does_not_match_uppercase_event() {
    // Lowercase ctrl bindings must NOT match uppercase events on legacy
    // terminals — that would break enhanced terminal dispatch where the
    // protocol correctly distinguishes case.
    let binding = parse_binding("ctrl+g").unwrap();
    let event = ev(KeyCode::Char('G'), KeyModifiers::CONTROL);

    reset_alt();
    assert!(
        !binding.matches(&event),
        "ctrl+g must NOT match Char('G')+CONTROL on a legacy terminal"
    );
}

#[test]
#[cfg(unix)]
fn legacy_terminal_pressed_returns_false_without_match() {
    let bindings = bind(&["ctrl+G"]);
    let event = ev(KeyCode::Char('x'), KeyModifiers::CONTROL);

    reset_alt();
    assert!(!pressed(&bindings, &event));
}

// ---- end legacy terminal tests -------------------------------------------

// ---- Windows CapsLock normalization ---------------------------------------
//
// crossterm's Windows backend derives a key event's reported char case from
// `shift_pressed XOR capslock_on`, so with CapsLock on, an unshifted letter
// arrives as an uppercase `KeyCode::Char` even though the `SHIFT` modifier
// bit is `false`. `KeyBinding::matches` must re-derive the letter's case from
// the `SHIFT` modifier alone (which crossterm reports independently of
// CapsLock) rather than trusting the char's case, or every lowercase-letter
// binding silently stops matching whenever CapsLock is on.

#[test]
#[cfg(not(unix))]
fn matches_ignores_capslock_for_lowercase_binding() {
    let b = parse_binding("ctrl+p").unwrap();
    // CapsLock on, no physical Shift: crossterm reports Char('P') + CONTROL
    // (no SHIFT bit). The lowercase-spec binding must still match.
    assert!(b.matches(&ev(KeyCode::Char('P'), KeyModifiers::CONTROL)));
}

#[test]
#[cfg(not(unix))]
fn matches_ignores_capslock_for_uppercase_binding() {
    let b = parse_binding("ctrl+P").unwrap();
    // CapsLock *and* physical Shift held together: shift_pressed XOR
    // capslock_on cancels out, so crossterm reports Char('p') + CONTROL +
    // SHIFT. The uppercase-spec (Ctrl+Shift+P) binding must still match.
    assert!(b.matches(&ev(
        KeyCode::Char('p'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    )));
}

#[test]
#[cfg(not(unix))]
fn matches_still_distinguishes_shift_without_capslock() {
    // No CapsLock involved: plain case-per-modifier behavior must be
    // preserved so `ctrl+f` and `ctrl+F` remain distinct bindings.
    let lower = parse_binding("ctrl+f").unwrap();
    let upper = parse_binding("ctrl+F").unwrap();

    assert!(lower.matches(&ev(KeyCode::Char('f'), KeyModifiers::CONTROL)));
    assert!(!upper.matches(&ev(KeyCode::Char('f'), KeyModifiers::CONTROL)));

    assert!(upper.matches(&ev(
        KeyCode::Char('F'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    )));
    assert!(!lower.matches(&ev(
        KeyCode::Char('F'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    )));
}

#[test]
fn keymap_has_toggle_raw_markdown() {
    let keymap = Keymap::default();
    assert!(
        pressed(
            &keymap.toggle_raw_markdown,
            &KeyEvent::new(KeyCode::Char('M'), KeyModifiers::SHIFT)
        ),
        "toggle_raw_markdown must match M",
    );
}

// ---- binding scopes, F-keys, and the editor-style defaults (issue #298) ----

#[test]
fn parses_scope_prefixes() {
    let t = parse_binding("tree:q").unwrap();
    assert_eq!(t.scope, BindingScope::Tree);
    assert_eq!(t.code, KeyCode::Char('q'));

    let c = parse_binding("content:ctrl+b").unwrap();
    assert_eq!(c.scope, BindingScope::Content);
    assert!(c.ctrl);
    assert_eq!(c.code, KeyCode::Char('b'));

    let g = parse_binding("ctrl+c").unwrap();
    assert_eq!(g.scope, BindingScope::Global);
}

#[test]
fn scoped_binding_roundtrips_through_serde() {
    for spec in ["tree:q", "content:ctrl+b", "ctrl+c", "F5", "tree:?"] {
        let b = parse_binding(spec).unwrap();
        let serialized = toml::Value::try_from(b).unwrap();
        let s = serialized.as_str().unwrap();
        let back = parse_binding(s).unwrap();
        assert_eq!(back.scope, b.scope, "scope must roundtrip for {spec}");
        assert_eq!(back.code, b.code, "code must roundtrip for {spec}");
        assert_eq!(back.ctrl, b.ctrl, "ctrl must roundtrip for {spec}");
    }
}

#[test]
fn parses_function_keys() {
    assert_eq!(parse_binding("F1").unwrap().code, KeyCode::F(1));
    assert_eq!(parse_binding("f5").unwrap().code, KeyCode::F(5));
    assert_eq!(parse_binding("F12").unwrap().code, KeyCode::F(12));
    assert!(parse_binding("F13").is_err());
    assert!(parse_binding("f0").is_err());
}

#[test]
fn function_key_display() {
    assert_eq!(parse_binding("F5").unwrap().display(), "F5");
    assert_eq!(parse_binding("ctrl+F5").unwrap().display(), "Ctrl+F5");
}

#[test]
fn pressed_in_honours_scope() {
    let binds = bind(&["ctrl+c", "tree:q"]);
    let q = ev(KeyCode::Char('q'), KeyModifiers::empty());
    assert!(pressed_in(&binds, &q, BindingScope::Tree));
    assert!(
        !pressed_in(&binds, &q, BindingScope::Content),
        "tree-scoped binding must not fire in the content pane"
    );
    let ctrl_c = ev(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(pressed_in(&binds, &ctrl_c, BindingScope::Content));
    assert!(pressed_in(&binds, &ctrl_c, BindingScope::Tree));
    // Scope-agnostic `pressed` matches regardless (overlay contexts).
    assert!(pressed(&binds, &q));
}

/// The content pane must stay free of bare letters (for future editing)
/// except the vim motion set, plus `M` and `o` — the bundled markdown plugin
/// only recognizes the literal key `M`, and `o` is used to open files
/// externally — plus `y`/`Y` for copy-line/copy-file clipboard operations.
/// Tree-structural actions are exempt: their bindings only dispatch from the
/// tree handler.
#[test]
fn default_content_reachable_letters_are_motions_only() {
    let motions = [
        'j', 'k', 'h', 'l', 'g', 'G', '0', 'n', 'N', 'M', 'o', 'y', 'Y', ' ',
    ];
    let tree_structural = [
        "tree_expand",
        "tree_collapse",
        "tree_collapse_all",
        "tree_expand_all",
    ];
    let keymap = Keymap::default();
    for action in crate::actions::ACTIONS {
        if tree_structural.contains(&action.id) {
            continue;
        }
        for b in keymap.bindings_for_action(action.id) {
            if b.scope == BindingScope::Tree || b.ctrl || b.alt || b.super_key {
                continue;
            }
            if let KeyCode::Char(c) = b.code {
                if !c.is_ascii_alphabetic() {
                    continue;
                }
                assert!(
                    motions.contains(&c),
                    "action '{}' binds bare '{}' reachable from the content pane",
                    action.id,
                    c
                );
            }
        }
    }
}

#[test]
fn macos_defaults_add_cmd_primaries_with_ctrl_fallbacks() {
    let mut map = Keymap::default();
    apply_macos_defaults(&mut map);
    let first = &map.find_files[0];
    assert!(first.super_key, "mac find_files primary must be cmd+p");
    assert_eq!(first.code, KeyCode::Char('p'));
    assert!(
        map.find_files.iter().any(|b| b.ctrl && !b.super_key),
        "ctrl+p fallback must remain for terminals that swallow cmd"
    );
    // goto_line stays on ctrl even on mac (matching mac VS Code).
    assert!(map.goto_line.iter().all(|b| !b.super_key));
}

// -- action id canonicalisation (issue #495) ---------------------------------
//
// `bindings_for_action` used to accept both an `x_picker`/`x_toggle`-style id
// (used by the keymap/help) and an `open_x`/`toggle_x`-style alias (used by
// the old hand-maintained command palette) for the same field. Those aliases
// are gone now that the palette derives from the same canonical
// `crate::actions::ACTIONS` ids the keymap already used.

#[test]
fn canonical_action_ids_resolve_bindings() {
    let keymap = Keymap::default();
    for id in [
        "help",
        "search_files",
        "search_content",
        "file_history",
        "theme_picker",
        "git_mode_toggle",
        "git_mode_flat_toggle",
        "recent_files",
        "plugin_picker",
        "goto_line",
    ] {
        assert!(
            !keymap.label_for_action(id).is_empty(),
            "canonical action id '{id}' must resolve to a bound key",
        );
    }
}

#[test]
fn old_palette_aliases_no_longer_resolve() {
    let keymap = Keymap::default();
    for alias in [
        "toggle_help",
        "open_file_search",
        "open_content_search",
        "open_file_history",
        "open_theme_picker",
        "toggle_git_mode",
        "toggle_git_flat",
        "toggle_word_wrap",
        "open_recent_files",
        "open_plugin_picker",
        "go_to_line",
    ] {
        assert!(
            keymap.label_for_action(alias).is_empty(),
            "'{alias}' is a removed alias and must no longer resolve to a binding",
        );
    }
}

// ---- legacy [keys] action renames (#553) -----------------------------------

#[test]
fn legacy_yaml_fold_toggle_folds_into_fold_toggle() {
    let toml_str = "yaml_fold_toggle = [\"space\"]\n";
    let mut keymap: Keymap = toml::from_str(toml_str).unwrap();
    keymap.migrate_legacy_keys();
    assert!(pressed(
        &keymap.fold_toggle,
        &ev(KeyCode::Char(' '), KeyModifiers::NONE)
    ));
    assert!(keymap.legacy_yaml_fold_toggle.is_none());
}

#[test]
fn legacy_visual_line_blame_folds_into_blame_line() {
    let toml_str = "visual_line_blame = [\"b\"]\n";
    let mut keymap: Keymap = toml::from_str(toml_str).unwrap();
    keymap.migrate_legacy_keys();
    assert!(pressed(
        &keymap.blame_line,
        &ev(KeyCode::Char('b'), KeyModifiers::NONE)
    ));
    assert!(keymap.legacy_visual_line_blame.is_none());
}

#[test]
fn legacy_key_wins_when_both_old_and_new_present() {
    let toml_str = "yaml_fold_toggle = [\"b\"]\nfold_toggle = [\"space\"]\n";
    let mut keymap: Keymap = toml::from_str(toml_str).unwrap();
    keymap.migrate_legacy_keys();
    assert!(pressed(
        &keymap.fold_toggle,
        &ev(KeyCode::Char('b'), KeyModifiers::NONE)
    ));
    assert!(!pressed(
        &keymap.fold_toggle,
        &ev(KeyCode::Char(' '), KeyModifiers::NONE)
    ));
}

#[test]
fn copy_line_default_binding_is_content_y() {
    let keymap = Keymap::default();
    assert!(
        pressed(
            &keymap.copy_line,
            &ev(KeyCode::Char('y'), KeyModifiers::NONE),
        ),
        "copy_line must match bare 'y' in content scope"
    );
}

#[test]
fn copy_file_default_binding_is_content_shift_y() {
    let keymap = Keymap::default();
    assert!(
        pressed(
            &keymap.copy_file,
            &ev(KeyCode::Char('Y'), KeyModifiers::NONE),
        ),
        "copy_file must match 'Y' in content scope"
    );
}

#[test]
fn removed_keymap_actions_parse_without_error_and_bind_nothing() {
    let toml_str = "visual_line_toggle = [\"V\"]\n";
    let mut keymap: Keymap = toml::from_str(toml_str).unwrap();
    keymap.migrate_legacy_keys();
    assert!(!pressed(
        &keymap.blame_line,
        &ev(KeyCode::Char('V'), KeyModifiers::NONE)
    ));
}

#[test]
fn default_keymap_includes_open_external() {
    let keymap = Keymap::default();
    assert!(pressed(
        &keymap.open_external,
        &ev(KeyCode::Char('o'), KeyModifiers::NONE)
    ));
}
