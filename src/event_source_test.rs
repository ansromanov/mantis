use std::collections::VecDeque;

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
// try_next_raw_event: non-blocking path
// ---------------------------------------------------------------------------

#[test]
fn try_next_raw_event_returns_buffered_event() {
    let mut src = RawEventSource::new();
    src.buf.extend_from_slice(b"q");
    let ev = src.try_next_raw_event().expect("must not error");
    assert!(
        ev.is_some(),
        "pre-buffered event must be returned immediately"
    );
}

#[test]
fn try_next_raw_event_returns_none_when_buffer_empty() {
    let mut src = RawEventSource::new();
    // poll(fd=0, timeout=0) returns immediately when stdin has no data.
    let ev = src.try_next_raw_event().expect("must not error");
    assert!(ev.is_none());
}

// ---------------------------------------------------------------------------
// fill_with / PollReader: mock-based tests for #456 (exact-4096 freeze)
// ---------------------------------------------------------------------------

/// Mock [`PollReader`] that returns pre-configured poll and read results
/// from front-to-back queues.
struct MockReader {
    poll_results: VecDeque<bool>,
    read_counts: VecDeque<usize>,
}

impl PollReader for MockReader {
    fn poll(&mut self, _timeout_ms: libc::c_int) -> io::Result<bool> {
        Ok(self.poll_results.pop_front().unwrap_or(false))
    }

    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(self.read_counts.pop_front().unwrap_or(0))
    }
}

#[test]
fn fill_with_exact_4096_does_not_block() {
    // Simulate an exact 4096-byte burst with no more data available.
    // The loop must use poll(0) and break, not issue a blocking read.
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, false]),
        read_counts: VecDeque::from([4096]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 4096);
    assert!(
        reader.read_counts.is_empty(),
        "no more reads should be attempted"
    );
}

#[test]
fn fill_with_partial_read_stops_without_extra_poll() {
    // A read returning less than the buffer size means no more data is
    // pending, so the loop must break without calling poll(0).
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, false]),
        read_counts: VecDeque::from([100]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 100);
    // The poll(0) result was not consumed.
    assert_eq!(
        reader.poll_results.len(),
        1,
        "poll(0) should not be consumed after partial read"
    );
}

#[test]
fn fill_with_drains_multiple_4096_bursts() {
    // Two exact 4096-byte reads, with poll(0) returning true between them.
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, true, false]),
        read_counts: VecDeque::from([4096, 4096]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 8192);
    assert!(reader.read_counts.is_empty());
}

#[test]
fn fill_with_returns_early_on_poll_timeout() {
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([false]),
        read_counts: VecDeque::from([]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert!(src.buf.is_empty());
}

#[test]
fn fill_with_empty_read_is_eof() {
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true]),
        read_counts: VecDeque::from([0]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert!(src.buf.is_empty());
}
