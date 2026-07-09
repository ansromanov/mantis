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

// ---- case-insensitive modifier+letter matching ------------------------------
//
// Ctrl+Shift combinations are unsupported (kitty reserves ctrl+shift as
// kitty_mod, Windows Terminal binds Ctrl+Shift+P/F itself, legacy terminals
// can't report them). Modifier+letter bindings therefore match
// case-insensitively and `parse_binding` normalizes them to lowercase, making
// them immune to CapsLock, a held Shift, and config-spec case.

#[test]
fn parse_normalizes_modifier_letter_to_lowercase() {
    assert_eq!(parse_binding("ctrl+P").unwrap().code, KeyCode::Char('p'));
    assert_eq!(
        parse_binding("ctrl+shift+p").unwrap().code,
        KeyCode::Char('p')
    );
    assert_eq!(parse_binding("cmd+F").unwrap().code, KeyCode::Char('f'));
    // Unmodified letters keep their case (Shift is encoded in char case).
    assert_eq!(parse_binding("G").unwrap().code, KeyCode::Char('G'));
}

#[test]
fn ctrl_binding_matches_either_case_event() {
    #[cfg(unix)]
    reset_alt();
    let b = parse_binding("ctrl+p").unwrap();
    // Plain press.
    assert!(b.matches(&ev(KeyCode::Char('p'), KeyModifiers::CONTROL)));
    // Shift held (enhanced terminal reports uppercase + SHIFT).
    assert!(b.matches(&ev(
        KeyCode::Char('P'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT
    )));
    // CapsLock on (uppercase char, no SHIFT bit).
    assert!(b.matches(&ev(KeyCode::Char('P'), KeyModifiers::CONTROL)));
}

#[test]
fn ctrl_binding_rejects_different_char() {
    #[cfg(unix)]
    reset_alt();
    let binding = parse_binding("ctrl+g").unwrap();
    assert!(!binding.matches(&ev(KeyCode::Char('x'), KeyModifiers::CONTROL)));
}

#[test]
#[cfg(unix)]
fn unmodified_binding_stays_case_sensitive() {
    // Non-modifier bindings (e.g. 'G' for content_bottom) must NOT become
    // case-insensitive — char case is how Shift is encoded for them.
    reset_alt();
    let binding = parse_binding("G").unwrap();
    assert!(!binding.matches(&ev(KeyCode::Char('g'), KeyModifiers::empty())));
}

#[test]
#[cfg(unix)]
fn ctrl_binding_matches_shifted_event_with_alt_keys() {
    // Enhanced terminal, Shift held: alt-keys report base 'p', event carries
    // SHIFT, so the base resolves to 'P' — the ctrl+p binding must still fire.
    let binding = parse_binding("ctrl+p").unwrap();
    let event = ev(
        KeyCode::Char('P'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    );

    set_alt(Some('P'), Some('p'));
    assert!(binding.matches(&event));
    reset_alt();
}

#[test]
#[cfg(not(unix))]
fn matches_ignores_capslock_for_lowercase_binding() {
    let b = parse_binding("ctrl+p").unwrap();
    // CapsLock on, no physical Shift: crossterm reports Char('P') + CONTROL
    // (no SHIFT bit). The lowercase-spec binding must still match.
    assert!(b.matches(&ev(KeyCode::Char('P'), KeyModifiers::CONTROL)));
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
/// externally — plus `y`/`Y` for copy-line/copy-file clipboard operations
/// and `B` for blame-line (the Shift pair of the `ctrl+b` blame toggle).
/// Tree-structural actions are exempt: their bindings only dispatch from the
/// tree handler.
#[test]
fn default_content_reachable_letters_are_motions_only() {
    let motions = [
        'j', 'k', 'h', 'l', 'g', 'G', '0', 'n', 'N', 'M', 'o', 'y', 'Y', 'B', ' ',
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
    assert!(first.super_key, "mac find_files primary must be cmd+t");
    assert_eq!(first.code, KeyCode::Char('t'));
    assert!(
        map.find_files.iter().any(|b| b.ctrl && !b.super_key),
        "ctrl+t fallback must remain for terminals that swallow cmd"
    );
    let palette = &map.command_palette[0];
    assert!(palette.super_key, "mac palette primary must be cmd+p");
    assert_eq!(palette.code, KeyCode::Char('p'));
    // goto_line stays on ctrl even on mac (matching mac VS Code).
    assert!(map.goto_line.iter().all(|b| !b.super_key));
}

// ---- stable cross-terminal defaults (no Ctrl+Shift) -------------------------

/// No default binding may use an uppercase letter together with a modifier
/// (the old Ctrl+Shift encoding) — those combos are reserved by kitty
/// (`kitty_mod`) and Windows Terminal, and legacy terminals can't report them.
#[test]
fn no_default_binding_uses_ctrl_shift() {
    let keymap = Keymap::default();
    for action in crate::actions::ACTIONS {
        for b in keymap.bindings_for_action(action.id) {
            if b.ctrl || b.alt || b.super_key {
                if let KeyCode::Char(c) = b.code {
                    assert!(
                        !c.is_ascii_uppercase(),
                        "action '{}' binds a modifier+uppercase (Ctrl+Shift-style) combo '{}'",
                        action.id,
                        b.display()
                    );
                }
            }
        }
    }
}

#[test]
fn stable_default_bindings() {
    #[cfg(unix)]
    reset_alt();
    let keymap = Keymap::default();
    let ctrl = |c: char| ev(KeyCode::Char(c), KeyModifiers::CONTROL);
    assert!(
        pressed(&keymap.command_palette, &ctrl('p')),
        "command palette must open on ctrl+p in any panel"
    );
    assert!(
        pressed(&keymap.search_content, &ctrl('f')),
        "content search must open on ctrl+f"
    );
    assert!(
        pressed(&keymap.find_files, &ctrl('t')),
        "file finder must open on ctrl+t"
    );
    assert!(
        pressed(&keymap.git_mode_toggle, &ctrl('d')),
        "git mode must toggle on ctrl+d"
    );
    assert!(
        pressed_in(
            &keymap.blame_line,
            &ev(KeyCode::Char('B'), KeyModifiers::SHIFT),
            BindingScope::Content
        ),
        "blame line must fire on Shift+B in the content pane"
    );
    // search_files is the contextual `/`: tree filter or in-file search.
    assert!(pressed_in(
        &keymap.search_files,
        &ev(KeyCode::Char('/'), KeyModifiers::empty()),
        BindingScope::Tree
    ));
    assert!(pressed_in(
        &keymap.search_files,
        &ev(KeyCode::Char('/'), KeyModifiers::empty()),
        BindingScope::Content
    ));
    assert!(
        !pressed_in(&keymap.search_files, &ctrl('f'), BindingScope::Content),
        "ctrl+f now belongs to search_content, not search_files"
    );
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

#[test]
fn action_for_key_resolves_correctly() {
    let keymap = Keymap::default();
    let key = ev(KeyCode::Char('o'), KeyModifiers::NONE);
    assert_eq!(
        keymap.action_for_key(&key, BindingScope::Content),
        Some("open_external")
    );
}

#[test]
fn tree_width_grow_default_binding_is_right_bracket() {
    let keymap = Keymap::default();
    let grow = ev(KeyCode::Char(']'), KeyModifiers::empty());
    assert!(
        pressed_in(&keymap.tree_width_grow, &grow, BindingScope::Tree),
        "tree_width_grow must bind to ']' in tree scope"
    );
    assert!(
        !pressed_in(&keymap.tree_width_grow, &grow, BindingScope::Content),
        "tree_width_grow must NOT fire in content scope"
    );
}

#[test]
fn tree_width_shrink_default_binding_is_left_bracket() {
    let keymap = Keymap::default();
    let shrink = ev(KeyCode::Char('['), KeyModifiers::empty());
    assert!(
        pressed_in(&keymap.tree_width_shrink, &shrink, BindingScope::Tree),
        "tree_width_shrink must bind to '[' in tree scope"
    );
    assert!(
        !pressed_in(&keymap.tree_width_shrink, &shrink, BindingScope::Content),
        "tree_width_shrink must NOT fire in content scope"
    );
}
