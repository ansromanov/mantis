use super::*;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind};

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

fn mouse_event(ev: &Event) -> &crossterm::event::MouseEvent {
    match ev {
        Event::Mouse(m) => m,
        other => panic!("expected Mouse event, got {other:?}"),
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
// Unhandled bytes: Ok(None) not Err
// ---------------------------------------------------------------------------

#[test]
fn high_byte_skipped_by_next_raw_event() {
    // parse_event returns Err for 0x80; next_raw_event must convert that to
    // Ok(None) (skip the byte) rather than propagating the error.
    let mut src = RawEventSource::new();
    src.buf.push(0x80);
    src.buf.push(b'q'); // a valid event after the bad byte
    let result = src.next_raw_event();
    assert!(
        result.is_ok(),
        "bad byte must not propagate Err from next_raw_event"
    );
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
// SGR mouse: '<' prefix stripped, cb parsed correctly
// ---------------------------------------------------------------------------

fn sgr_press(cb: u16, col: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<{cb};{col};{row}M").into_bytes()
}

fn sgr_release(cb: u16, col: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<{cb};{col};{row}m").into_bytes()
}

#[test]
fn sgr_left_click_press() {
    let (ev, _) = parse_ok(&sgr_press(0, 5, 10));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Left));
    assert_eq!(m.column, 4); // 1-indexed → 0-indexed
    assert_eq!(m.row, 9);
}

#[test]
fn sgr_left_click_release() {
    let (ev, _) = parse_ok(&sgr_release(0, 5, 10));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Up(MouseButton::Left));
}

#[test]
fn sgr_right_click_press() {
    let (ev, _) = parse_ok(&sgr_press(2, 1, 1));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Right));
}

#[test]
fn sgr_middle_click_press() {
    let (ev, _) = parse_ok(&sgr_press(1, 1, 1));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Middle));
}

#[test]
fn sgr_scroll_up() {
    // cb=64 (0x40) = scroll, bit 0 clear = ScrollUp
    let (ev, _) = parse_ok(&sgr_press(64, 1, 1));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::ScrollUp);
}

#[test]
fn sgr_scroll_down() {
    // cb=65 (0x41) = scroll, bit 0 set = ScrollDown
    let (ev, _) = parse_ok(&sgr_press(65, 1, 1));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::ScrollDown);
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
// CSI-u: base key stored in thread-local
// ---------------------------------------------------------------------------

#[test]
fn csi_u_sets_current_base_key() {
    // U+0437 = 'з' (decimal 1079), alternate = 112 = 'p'
    // ESC [ 1079 : 112 ; 1 u → primary='з', base_key=Some(Char('p'))
    CURRENT_BASE_KEY.with(|c| c.set(None));
    let seq = b"\x1b[1079:112;1u";
    let (ev, _) = parse_ok(seq);
    let k = key_event(&ev);
    assert_eq!(k.code, KeyCode::Char('з'));
    let base = CURRENT_BASE_KEY.with(|c| c.get());
    assert_eq!(base, Some(KeyCode::Char('p')));
    CURRENT_BASE_KEY.with(|c| c.set(None));
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
// SGR mouse drag vs moved
// ---------------------------------------------------------------------------

#[test]
fn sgr_left_button_drag() {
    // cb=32 (0x20): bit5=motion, bits0-1=0 (left button held) → Drag(Left)
    let (ev, _) = parse_ok(&sgr_press(32, 10, 5));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Drag(MouseButton::Left));
}

#[test]
fn sgr_motion_no_button_is_moved() {
    // cb=35 (0x23): bit5=motion, bits0-1=3 (no button) → Moved
    let (ev, _) = parse_ok(&sgr_press(35, 10, 5));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Moved);
}

// ---------------------------------------------------------------------------
// Regular arrow codepoints (57350-57353)
// ---------------------------------------------------------------------------

#[test]
fn csi_u_regular_up_arrow() {
    let seq = b"\x1b[57352;1u";
    let (ev, _) = parse_ok(seq);
    assert_eq!(key_event(&ev).code, KeyCode::Up);
}

#[test]
fn csi_u_regular_down_arrow() {
    let seq = b"\x1b[57353;1u";
    let (ev, _) = parse_ok(seq);
    assert_eq!(key_event(&ev).code, KeyCode::Down);
}

#[test]
fn csi_u_regular_left_arrow() {
    let seq = b"\x1b[57350;1u";
    let (ev, _) = parse_ok(seq);
    assert_eq!(key_event(&ev).code, KeyCode::Left);
}

#[test]
fn csi_u_regular_right_arrow() {
    let seq = b"\x1b[57351;1u";
    let (ev, _) = parse_ok(seq);
    assert_eq!(key_event(&ev).code, KeyCode::Right);
}

// ---------------------------------------------------------------------------
// Double-movement prevention: params-present CSI arrows → Null
// ---------------------------------------------------------------------------

#[test]
fn csi_arrow_with_params_produces_null() {
    // ESC[1;1A = kitty DISAMBIGUATE_ESCAPE_CODES form of plain Up.
    // Must produce Null (not Up) so the CSI-u form is the sole event.
    let (ev, _) = parse_ok(b"\x1b[1;1A");
    assert_eq!(key_event(&ev).code, KeyCode::Null);
}

#[test]
fn csi_bare_up_arrow_still_works() {
    // Plain ESC[A from legacy terminals (no params) must still fire.
    let (ev, _) = parse_ok(b"\x1b[A");
    assert_eq!(key_event(&ev).code, KeyCode::Up);
}
