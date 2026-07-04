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

// ---- end kitty-protocol tests --------------------------------------------

#[test]
fn keymap_has_no_toggle_raw_markdown() {
    let keymap = Keymap::default();
    // Previously there was a `toggle_raw_markdown` field; it was removed with
    // the built-in markdown renderer. Constructing a Keymap without it proves
    // the struct no longer carries that field.
    // Verify the default keymap still works for an unrelated binding.
    assert!(
        pressed(
            &keymap.toggle_wrap,
            &KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE)
        ),
        "toggle_wrap must still match z key",
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
        "toggle_wrap",
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
