//! SGR-format mouse event decoding.
//!
//! Parses SGR-encoded mouse escape sequences (from ESC[<...) to translate terminal
//! grid coordinates, mouse buttons, scroll actions, dragging states, and keyboard
//! modifiers into structured crossterm `Event::Mouse` events.
//!
//! This module decouples mouse-specific parsing details from the generic keyboard and
//! escape sequence parser.
//!
//! Public (crate) items:
//! - [`parse_sgr_mouse`]: Translates an SGR mouse sequence into a mouse event.
//! - [`decode_mouse_button`]: Decodes a raw button code to a `MouseButton`.

use crossterm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::io;

/// Parse an SGR-encoded mouse event.
///
/// Format: `code;col;row` (parameters between '[' and the final M/m).
pub(crate) fn parse_sgr_mouse(
    params: &str,
    consumed: usize,
    default_kind: MouseEventKind,
) -> io::Result<Option<(Event, usize)>> {
    // SGR sequences include a leading '<' in the parameter string (ESC[<cb;col;rowM).
    let params = params.strip_prefix('<').unwrap_or(params);
    let mut parts = params.split(';');
    let cb_str = parts.next().unwrap_or("0");
    let col_str = parts.next().unwrap_or("1");
    let row_str = parts.next().unwrap_or("1");

    let raw_cb: u16 = cb_str.parse().unwrap_or(0);
    let col: u16 = col_str.parse().unwrap_or(0);
    let row: u16 = row_str.parse().unwrap_or(0);

    // SGR mouse modifier bits in cb: bit 2=Shift (0x04), bit 3=Meta (0x08),
    // bit 4=Ctrl (0x10). Extract and mask out before button/motion decode.
    let mut modifiers = KeyModifiers::empty();
    if raw_cb & 0x04 != 0 {
        modifiers |= KeyModifiers::SHIFT;
    }
    if raw_cb & 0x10 != 0 {
        modifiers |= KeyModifiers::CONTROL;
    }
    // Meta (bit 3) maps to Alt in the terminal world.
    if raw_cb & 0x08 != 0 {
        modifiers |= KeyModifiers::ALT;
    }
    let cb = raw_cb & !0b11100;

    // Button 0-2: press/release determined by final byte (M/m) via default_kind;
    // 0x20: motion (drag); 0x40: scroll (wheel).
    let kind = if cb & 0x40 != 0 {
        // Scroll event
        if cb & 0x01 != 0 {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        }
    } else if cb & 0x20 != 0 {
        // Bit 5 set = motion event. Bits 0-1 encode which button is held:
        // value 3 means no button (hover/move); anything else is a drag.
        if cb & 0x03 == 3 {
            MouseEventKind::Moved
        } else {
            MouseEventKind::Drag(decode_mouse_button(cb))
        }
    } else {
        // In SGR mode press vs release is signaled by the final byte (M or m),
        // conveyed here via default_kind. Preserve the correct button from cb.
        let button = decode_mouse_button(cb);
        match default_kind {
            MouseEventKind::Up(_) => MouseEventKind::Up(button),
            _ => MouseEventKind::Down(button),
        }
    };

    Ok(Some((
        Event::Mouse(MouseEvent {
            kind,
            column: col.saturating_sub(1),
            row: row.saturating_sub(1),
            modifiers,
        }),
        consumed,
    )))
}

pub(crate) fn decode_mouse_button(cb: u16) -> MouseButton {
    match cb & 0x03 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

#[cfg(test)]
#[path = "mouse_test.rs"]
mod tests;
