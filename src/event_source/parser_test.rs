use super::super::{AltKeys, RawEventSource, CURRENT_ALT_KEYS};
use super::*;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse(bytes: &[u8]) -> Option<(Event, usize)> {
    parse_event(bytes).unwrap()
}

fn parse_ok(bytes: &[u8]) -> (Event, usize) {
    parse(bytes).expect("expected a complete event")
}

fn key_event(ev: &Event) -> &crossterm::event::KeyEvent {
    match ev {
        Event::Key(k) => k,
        other => panic!("expected Key event, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// ctrl-byte range: 0x07 (BEL) → Ctrl+G
// ---------------------------------------------------------------------------

#[test]
fn bel_byte_produces_ctrl_g_not_error() {
    let (ev, consumed) = parse_ok(&[0x07]);
    assert_eq!(consumed, 1);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('g'));
    assert!(k.modifiers.contains(KeyModifiers::CONTROL));
}

// ---------------------------------------------------------------------------
// F-key arithmetic in parse_csi_tilde
// ---------------------------------------------------------------------------

fn csi_tilde(num: u16) -> Vec<u8> {
    // ESC [ <num> ~
    format!("\x1b[{num}~").into_bytes()
}

#[test]
fn fkey_range_11_to_15_correct() {
    let expected = [
        KeyCode::F(1),
        KeyCode::F(2),
        KeyCode::F(3),
        KeyCode::F(4),
        KeyCode::F(5),
    ];
    for (i, &code) in expected.iter().enumerate() {
        let (ev, _) = parse_ok(&csi_tilde(11 + i as u16));
        assert_eq!(key_event(&ev).code, code, "CSI {}~", 11 + i);
    }
}

#[test]
fn fkey_range_17_to_21_is_f6_to_f10() {
    let expected = [
        KeyCode::F(6),
        KeyCode::F(7),
        KeyCode::F(8),
        KeyCode::F(9),
        KeyCode::F(10),
    ];
    for (i, &code) in expected.iter().enumerate() {
        let (ev, _) = parse_ok(&csi_tilde(17 + i as u16));
        assert_eq!(key_event(&ev).code, code, "CSI {}~", 17 + i);
    }
}

#[test]
fn fkey_range_23_to_26_is_f11_to_f14() {
    let expected = [
        KeyCode::F(11),
        KeyCode::F(12),
        KeyCode::F(13),
        KeyCode::F(14),
    ];
    for (i, &code) in expected.iter().enumerate() {
        let (ev, _) = parse_ok(&csi_tilde(23 + i as u16));
        assert_eq!(key_event(&ev).code, code, "CSI {}~", 23 + i);
    }
}

// ---------------------------------------------------------------------------
// Incomplete sequence: buffer not cleared on Ok(None)
// ---------------------------------------------------------------------------

#[test]
fn incomplete_csi_returns_none_not_error() {
    // ESC [ without final byte = incomplete
    let result = parse_event(b"\x1b[");
    assert!(result.is_ok());
    assert!(result.unwrap().is_none());
}

// ---------------------------------------------------------------------------
// CSI-u: alternate keys stored in thread-local
// ---------------------------------------------------------------------------

#[test]
fn csi_u_stores_shifted_and_base_in_alt_keys() {
    // U+0437 = 'з' (decimal 1079), shifted = 1047 = 'З', base = 112 = 'p'
    // ESC [ 1079 : 1047 : 112 ; 1 u → 3-field form (real kitty protocol).
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
    let seq = b"\x1b[1079:1047:112;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('з'));
    let alt = CURRENT_ALT_KEYS.with(|c| c.get());
    assert_eq!(alt.shifted, Some('З'));
    assert_eq!(alt.base, Some('p'));
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
}

#[test]
fn csi_u_shifted_and_base_same_in_2_field_form() {
    // 2-field form: 1079:112 (primary='з', alternate='p').
    // The alternate is stored as shifted; base is None.
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
    let seq = b"\x1b[1079:112;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('з'));
    let alt = CURRENT_ALT_KEYS.with(|c| c.get());
    assert_eq!(alt.shifted, Some('p'));
    assert_eq!(alt.base, None);
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
}

#[test]
fn csi_u_no_shifted_field_middle_empty() {
    // Empty middle field: 1080::98 (primary='и', shifted absent, base='b').
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
    let seq = b"\x1b[1080::98;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('и'));
    let alt = CURRENT_ALT_KEYS.with(|c| c.get());
    assert_eq!(alt.shifted, None);
    assert_eq!(alt.base, Some('b'));
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
}

#[test]
fn csi_u_no_alternates_single_field() {
    // Single field: just 'k' (107). Both shifted and base are None.
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
    let seq = b"\x1b[107;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('k'));
    let alt = CURRENT_ALT_KEYS.with(|c| c.get());
    assert_eq!(alt.shifted, None);
    assert_eq!(alt.base, None);
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
}

// ---------------------------------------------------------------------------
// CSI-u: event kind (press / repeat / release)
// ---------------------------------------------------------------------------

#[test]
fn csi_u_repeat_event() {
    // ESC [ 112 ; 1 : 2 u → kind=Repeat
    let seq = b"\x1b[112;1:2u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.kind, KeyEventKind::Repeat);
}

#[test]
fn csi_u_release_event() {
    // ESC [ 112 ; 1 : 3 u → kind=Release
    let seq = b"\x1b[112;1:3u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.kind, KeyEventKind::Release);
}

// ---------------------------------------------------------------------------
// decode_modifier_mask: kitty encodes as bitmask+1
// ---------------------------------------------------------------------------

#[test]
fn csi_u_no_modifier_is_empty() {
    // modifier=1 means "no modifier" in kitty (bitmask 0 + 1)
    let seq = b"\x1b[107;1u"; // 'k', no modifier
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('k'));
    assert!(
        k.modifiers.is_empty(),
        "modifier=1 must decode as empty, not SHIFT"
    );
}

#[test]
fn csi_u_shift_modifier_decodes_correctly() {
    // modifier=2 means shift (bitmask 1 + 1). '?' = keycode 63.
    let seq = b"\x1b[63;2u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('?'));
    assert!(k.modifiers.contains(KeyModifiers::SHIFT));
    assert!(!k.modifiers.contains(KeyModifiers::ALT));
}

#[test]
fn csi_u_ctrl_modifier_decodes_correctly() {
    // modifier=5 means ctrl (bitmask 4 + 1)
    let seq = b"\x1b[112;5u"; // 'p' + ctrl
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    assert!(!k.modifiers.contains(KeyModifiers::SHIFT));
}

// ---------------------------------------------------------------------------
// CSI-u: shifted alternate substituted into KeyCode::Char when SHIFT is set
// (regression test for #519 - shifted ASCII symbols on kitty-protocol
// terminals were emitted as their unshifted digit/base character).
// ---------------------------------------------------------------------------

#[test]
fn csi_u_shift_uses_shifted_alternate_for_keycode() {
    // Shift+8 on a US layout: primary='8' (56), shifted='*' (42), modifier=2 (shift).
    let seq = b"\x1b[56:42;2u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('*'));
    assert!(k.modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn csi_u_no_shift_keeps_primary_keycode() {
    // Same codes as above but without the shift modifier: keycode stays '8'.
    let seq = b"\x1b[56:42;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('8'));
    assert!(!k.modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn csi_u_shift_without_shifted_field_falls_back_to_primary() {
    // Shift set but no shifted alternate reported: keep the primary codepoint.
    let seq = b"\x1b[112;2u"; // 'p' + shift, no alternate field
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('p'));
    assert!(k.modifiers.contains(KeyModifiers::SHIFT));
}

// ---------------------------------------------------------------------------
// CSI arrows: bare, disambiguated, and release forms.
// REPORT_ALL_KEYS_AS_ESCAPE_CODES only affects printable keys; arrows still
// come through the legacy CSI path (ESC[A or ESC[1;1A via DISAMBIGUATE_ESCAPE_CODES).
// With REPORT_EVENT_TYPES the terminal also sends ESC[1;1:3A on key release;
// that must decode as Release so handle_key can filter it (no double move).
// ---------------------------------------------------------------------------

#[test]
fn csi_bare_up_arrow() {
    let (ev, _) = parse_ok(b"\x1b[A");
    assert_eq!(key_event(&ev).code, KeyCode::Up);
    assert_eq!(key_event(&ev).kind, KeyEventKind::Press);
}

#[test]
fn csi_disambiguated_up_arrow() {
    // ESC[1;1A = DISAMBIGUATE_ESCAPE_CODES form of plain Up (no modifiers).
    let (ev, _) = parse_ok(b"\x1b[1;1A");
    assert_eq!(key_event(&ev).code, KeyCode::Up);
    assert_eq!(key_event(&ev).kind, KeyEventKind::Press);
}

#[test]
fn csi_arrow_release_decodes_as_release() {
    // ESC[1;1:3A = REPORT_EVENT_TYPES release event. Must be Release so
    // handle_key ignores it and the key fires exactly once per physical press.
    let (ev, _) = parse_ok(b"\x1b[1;1:3A");
    assert_eq!(key_event(&ev).code, KeyCode::Up);
    assert_eq!(key_event(&ev).kind, KeyEventKind::Release);
}

// ---------------------------------------------------------------------------
// CSI ~ with modifier params (the key bug in #455)
// ---------------------------------------------------------------------------

#[test]
fn csi_tilde_ctrl_pageup() {
    // ESC [ 5 ; 5 ~ → Ctrl+PageUp (modifier=5 = 4+1 = Ctrl)
    let (ev, _) = parse_ok(b"\x1b[5;5~");
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::PageUp);
    assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    assert!(!k.modifiers.contains(KeyModifiers::SHIFT));
}

#[test]
fn csi_tilde_shift_delete() {
    // ESC [ 3 ; 2 ~ → Shift+Delete (modifier=2 = 1+1 = Shift)
    let (ev, _) = parse_ok(b"\x1b[3;2~");
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Delete);
    assert!(k.modifiers.contains(KeyModifiers::SHIFT));
    assert!(!k.modifiers.contains(KeyModifiers::CONTROL));
}

#[test]
fn csi_tilde_ctrl_pageup_release() {
    // ESC [ 5 ; 5 : 3 ~ → Ctrl+PageUp with event type 3 (Release)
    let (ev, _) = parse_ok(b"\x1b[5;5:3~");
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::PageUp);
    assert!(k.modifiers.contains(KeyModifiers::CONTROL));
    assert_eq!(k.kind, KeyEventKind::Release);
}

#[test]
fn csi_tilde_plain_still_works() {
    // Plain PageUp without modifier must still decode correctly.
    let (ev, _) = parse_ok(b"\x1b[5~");
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::PageUp);
    assert!(k.modifiers.is_empty());
    assert_eq!(k.kind, KeyEventKind::Press);
}

// ---------------------------------------------------------------------------
// Multi-byte UTF-8 pasted text (#454): non-ASCII bytes must decode to Char,
// not be dropped one byte per tick.
// ---------------------------------------------------------------------------

#[test]
fn utf8_two_byte_char_decodes() {
    // 'é' = U+00E9 = 0xC3 0xA9
    let (ev, consumed) = parse_ok("é".as_bytes());
    assert_eq!(consumed, 2);
    assert_eq!(key_event(&ev).code, KeyCode::Char('é'));
}

#[test]
fn utf8_three_byte_char_decodes() {
    // 'мир' third char: 'р' = U+0440 = 0xD1 0x80 — use a true 3-byte char instead: '€' = U+20AC.
    let (ev, consumed) = parse_ok("€".as_bytes());
    assert_eq!(consumed, 3);
    assert_eq!(key_event(&ev).code, KeyCode::Char('€'));
}

#[test]
fn utf8_four_byte_char_decodes() {
    // '😀' = U+1F600, 4-byte UTF-8 sequence.
    let (ev, consumed) = parse_ok("😀".as_bytes());
    assert_eq!(consumed, 4);
    assert_eq!(key_event(&ev).code, KeyCode::Char('😀'));
}

#[test]
fn utf8_cyrillic_char_decodes() {
    // 'м' = U+043C, 2-byte UTF-8 sequence.
    let (ev, consumed) = parse_ok("м".as_bytes());
    assert_eq!(consumed, 2);
    assert_eq!(key_event(&ev).code, KeyCode::Char('м'));
}

#[test]
fn utf8_incomplete_two_byte_returns_none() {
    // Lead byte only, continuation byte not yet arrived.
    let result = parse_event(&[0xC3]).unwrap();
    assert!(result.is_none());
}

#[test]
fn utf8_incomplete_three_byte_returns_none() {
    let full = "€".as_bytes();
    let result = parse_event(&full[..2]).unwrap();
    assert!(result.is_none());
}

#[test]
fn utf8_incomplete_four_byte_returns_none() {
    let full = "😀".as_bytes();
    for n in 1..4 {
        let result = parse_event(&full[..n]).unwrap();
        assert!(result.is_none(), "expected None with {n} of 4 bytes");
    }
}

#[test]
fn utf8_paste_split_across_fill_boundary() {
    // Simulate the lead byte and continuation byte of 'é' (0xC3 0xA9)
    // arriving in separate reads, as would happen if a paste straddles two
    // poll/read cycles. parse_next must wait rather than error/skip.
    let full = "héllo мир".as_bytes();
    let split = 2; // "h" + 0xC3 (é's lead byte only), splitting 'é' mid-sequence
    let mut src = RawEventSource::new();

    src.buf.extend_from_slice(&full[..split]);
    let mut decoded = String::new();
    while let Some(ev) = src.parse_next().unwrap() {
        if let Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char(c),
            ..
        }) = ev
        {
            decoded.push(c);
        }
    }
    // Only 'h' should have decoded so far; the split 'é' must still be buffered.
    assert_eq!(decoded, "h");
    assert_eq!(src.buf.len() - src.pos, 1);

    // The rest of the paste arrives in a second read.
    src.buf.extend_from_slice(&full[split..]);
    while let Some(ev) = src.parse_next().unwrap() {
        if let Event::Key(crossterm::event::KeyEvent {
            code: KeyCode::Char(c),
            ..
        }) = ev
        {
            decoded.push(c);
        }
    }
    assert_eq!(decoded, "héllo мир");
}
