use super::super::parser::parse_event;
use crossterm::event::{Event, MouseButton, MouseEventKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse(bytes: &[u8]) -> Option<(Event, usize)> {
    parse_event(bytes).unwrap()
}

fn parse_ok(bytes: &[u8]) -> (Event, usize) {
    parse(bytes).expect("expected a complete event")
}

fn mouse_event(ev: &Event) -> &crossterm::event::MouseEvent {
    match ev {
        Event::Mouse(m) => m,
        other => panic!("expected Mouse event, got {other:?}"),
    }
}

fn sgr_press(cb: u16, col: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<{cb};{col};{row}M").into_bytes()
}

fn sgr_release(cb: u16, col: u16, row: u16) -> Vec<u8> {
    format!("\x1b[<{cb};{col};{row}m").into_bytes()
}

// ---------------------------------------------------------------------------
// SGR mouse: '<' prefix stripped, cb parsed correctly
// ---------------------------------------------------------------------------

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
// SGR mouse with modifier bits (#455)
// ---------------------------------------------------------------------------

#[test]
fn sgr_ctrl_click_press() {
    // cb=16 (0x10): bit 4 = Ctrl modifier.
    // After masking out modifier bits, cb=0 → Left button.
    let (ev, _) = parse_ok(&sgr_press(16, 5, 10));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Left));
    assert!(m
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL));
}

#[test]
fn sgr_shift_click_press() {
    // cb=4 (0x04): bit 2 = Shift modifier. After masking, cb=0 → Left.
    let (ev, _) = parse_ok(&sgr_press(4, 5, 10));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Left));
    assert!(m.modifiers.contains(crossterm::event::KeyModifiers::SHIFT));
}

#[test]
fn sgr_ctrl_shift_click_press() {
    // cb=20 (0x14): bit 2 (Shift) + bit 4 (Ctrl). After masking, cb=0 → Left.
    let (ev, _) = parse_ok(&sgr_press(20, 5, 10));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Down(MouseButton::Left));
    assert!(m.modifiers.contains(crossterm::event::KeyModifiers::SHIFT));
    assert!(m
        .modifiers
        .contains(crossterm::event::KeyModifiers::CONTROL));
}

#[test]
fn sgr_modifiers_dont_affect_scroll() {
    // cb=68 (0x44): scroll bit (0x40) + shift modifier bit (0x04).
    // After masking out modifier, cb=64 → ScrollUp.
    let (ev, _) = parse_ok(&sgr_press(68, 1, 1));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::ScrollUp);
    assert!(m.modifiers.contains(crossterm::event::KeyModifiers::SHIFT));
}

#[test]
fn sgr_modifiers_dont_affect_drag() {
    // cb=36 (0x24): motion bit (0x20) + shift modifier bit (0x04).
    // After masking out modifier bits, cb=32 → Drag(Left).
    let (ev, _) = parse_ok(&sgr_press(36, 10, 5));
    let m = mouse_event(&ev);
    assert_eq!(m.kind, MouseEventKind::Drag(MouseButton::Left));
    assert!(m.modifiers.contains(crossterm::event::KeyModifiers::SHIFT));
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
