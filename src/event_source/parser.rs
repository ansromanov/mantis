//! Low-level terminal byte stream parser for events.
//!
//! This module contains parsing functions that translate raw ESC sequences, CSI
//! parameters, and UTF-8 byte patterns into structured crossterm `Event` objects.
//! It supports standard ANSI escapes, function key tilde sequences, and the Kitty
//! keyboard protocol.
//!
//! Public (crate) items:
//! - [`parse_event`]: Parses a single event from raw bytes.
//! - [`decode_modifier_mask`]: Decodes Kitty protocol modifier bitmask.

use std::io;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton,
    MouseEventKind,
};

use super::mouse::parse_sgr_mouse;
use super::{AltKeys, CURRENT_ALT_KEYS};

/// Try to parse a single event from the front of `bytes`.
///
/// Returns `Ok(Some((event, consumed)))` on success, `Ok(None)` for an
/// incomplete sequence (need more data), or `Err` for an unparseable byte.
pub(crate) fn parse_event(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    if bytes.is_empty() {
        return Ok(None);
    }

    match bytes[0] {
        0x1B => parse_escape(bytes),
        // Ctrl+@ through Ctrl+_ (and DEL / Ctrl+?)
        // 0x09 = Tab, 0x0D = CR, 0x1B = ESC — handled individually below.
        0x00..=0x08 | 0x0A | 0x0C | 0x0E..=0x1A | 0x1C..=0x1F => {
            let code = ctrl_byte_to_code(bytes[0]);
            Ok(Some((
                Event::Key(KeyEvent::new(code, KeyModifiers::CONTROL)),
                1,
            )))
        }
        0x0B => {
            // Ctrl+K
            Ok(Some((
                Event::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL)),
                1,
            )))
        }
        // Tab / Ctrl+I / Ctrl+J (newline in raw mode)
        0x09 => Ok(Some((
            Event::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::empty())),
            1,
        ))),
        // Carriage return -> Enter
        0x0D => Ok(Some((
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())),
            1,
        ))),
        // DEL -> Backspace (some terminals)
        0x7F => Ok(Some((
            Event::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty())),
            1,
        ))),
        // Printable ASCII
        0x20..=0x7E => Ok(Some((
            Event::Key(KeyEvent::new(
                KeyCode::Char(bytes[0] as char),
                KeyModifiers::empty(),
            )),
            1,
        ))),
        // Multi-byte UTF-8 lead byte (2/3/4-byte sequences). Pasted text
        // arrives as raw UTF-8 rather than CSI-u, so it must be decoded here
        // or every non-ASCII character is silently dropped (#454).
        0xC2..=0xF4 => parse_utf8_char(bytes),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unhandled byte 0x{:02X}", bytes[0]),
        )),
    }
}

/// Decode a multi-byte UTF-8 character starting at `bytes[0]`.
///
/// Returns `Ok(None)` if the sequence is not yet fully buffered (need more
/// data from the next `fill()`), or `Err` if the continuation bytes are
/// invalid UTF-8.
fn parse_utf8_char(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    let lead = bytes[0];
    let len = if lead & 0xE0 == 0xC0 {
        2
    } else if lead & 0xF0 == 0xE0 {
        3
    } else {
        4
    };

    if bytes.len() < len {
        return Ok(None); // incomplete sequence — wait for more bytes
    }

    match std::str::from_utf8(&bytes[..len]) {
        Ok(s) => {
            let c = s.chars().next().ok_or_else(|| invalid("empty utf8"))?;
            Ok(Some((
                Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty())),
                len,
            )))
        }
        Err(_) => Err(invalid("invalid utf8 sequence")),
    }
}

/// Interpret a control byte (0x00-0x1F, excluding tab/lf/cr/esc) as a
/// Ctrl+letter combination.
fn ctrl_byte_to_code(b: u8) -> KeyCode {
    // Ctrl+letter maps bytes 0x01-0x1A to letters A-Z / a-z.
    // 0x00 = Ctrl+Space, 0x1B = Escape (handled elsewhere), etc.
    match b {
        0x00 => KeyCode::Char(' '), // Ctrl+Space / Ctrl+@
        0x01..=0x1A => {
            // 0x01 = Ctrl+A, 0x02 = Ctrl+B, … 0x1A = Ctrl+Z
            let c = (b + 0x60) as char; // 0x01 + 0x60 = 'a'
            KeyCode::Char(c)
        }
        0x1C => KeyCode::Char('\\'), // Ctrl+\
        0x1D => KeyCode::Char(']'),  // Ctrl+]
        0x1E => KeyCode::Char('^'),  // Ctrl+^
        0x1F => KeyCode::Char('_'),  // Ctrl+_
        _ => KeyCode::Null,          // unreachable from the match above
    }
}

/// Parse a sequence starting with ESC (0x1B).
fn parse_escape(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    if bytes.len() < 2 {
        return Ok(None); // need at least one more byte
    }

    match bytes[1] {
        b'[' => parse_csi(bytes),
        b'O' => parse_ss3(bytes),
        // Plain ESC
        _ => Ok(Some((
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())),
            // Consume just the ESC byte; the next byte will be parsed as a
            // separate event (Alt prefix in some terminals).
            1,
        ))),
    }
}

/// Find the end of a CSI sequence: any byte 0x40-0x7E is a final byte.
fn csi_final_byte_pos(bytes: &[u8]) -> Option<usize> {
    // Position 0 is ESC, position 1 is '[' — final byte must be at index ≥ 2.
    // Use enumerate+skip+find to get the absolute index.
    bytes[2..]
        .iter()
        .position(|b| *b >= 0x40 && *b <= 0x7E)
        .map(|i| i + 2)
}

fn parse_csi(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    let final_pos = match csi_final_byte_pos(bytes) {
        Some(p) => p,
        None => return Ok(None), // incomplete
    };
    let final_byte = bytes[final_pos];
    let consumed = final_pos + 1;

    let params = &bytes[2..final_pos]; // between '[' and final byte
    let params_str = match std::str::from_utf8(params) {
        Ok(s) => s,
        Err(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "non-utf8 CSI params",
            ))
        }
    };

    match final_byte {
        b'u' => parse_csi_u(params_str, consumed),
        b'A' => csi_arrow(KeyCode::Up, params_str, consumed),
        b'B' => csi_arrow(KeyCode::Down, params_str, consumed),
        b'C' => csi_arrow(KeyCode::Right, params_str, consumed),
        b'D' => csi_arrow(KeyCode::Left, params_str, consumed),
        b'H' => Ok(Some((key(KeyCode::Home), consumed))),
        b'F' => Ok(Some((key(KeyCode::End), consumed))),
        b'Z' => Ok(Some((key(KeyCode::BackTab), consumed))),
        b'~' => parse_csi_tilde(params_str, consumed),
        b'M' => parse_sgr_mouse(
            params_str,
            consumed,
            MouseEventKind::Down(MouseButton::Left),
        ),
        b'm' => parse_sgr_mouse(params_str, consumed, MouseEventKind::Up(MouseButton::Left)),
        b'I' => Ok(Some((Event::FocusGained, consumed))),
        b'O' => Ok(Some((Event::FocusLost, consumed))),
        // Unhandled CSI sequence – just consume it so we don't get stuck.
        _ => Ok(Some((key(KeyCode::Null), consumed))),
    }
}

/// Parse a CSI-u key event.
///
/// Format: `keycode[:shifted[:base-layout]][; modifiers[:event_type]]`
///
/// We extract both the **shifted** key (field 1) and the **base-layout** key
/// (field 2, the US-physical key) and store them in [`CURRENT_ALT_KEYS`].
fn parse_csi_u(params: &str, consumed: usize) -> io::Result<Option<(Event, usize)>> {
    let mut parts = params.split(';');

    // ---- key codes (primary[:shifted[:base-layout]]) ----
    let codes_str = parts.next().unwrap_or("");
    let mut codes = codes_str.split(':');

    let primary_str = codes.next().unwrap_or("0");
    let primary: u32 = primary_str
        .parse()
        .map_err(|_| invalid("bad primary keycode"))?;

    let parse_char = |s: &str| s.parse::<u32>().ok().and_then(char::from_u32);
    // Field 1 = shifted (current layout), field 2 = base-layout (US physical).
    // Either may be empty, e.g. "98::100" when there is no shifted variant.
    let shifted = codes.next().and_then(parse_char);
    let base = codes.next().and_then(parse_char);

    // ---- modifiers and event type ----
    let mut modifiers = KeyModifiers::empty();
    let mut kind = KeyEventKind::Press;

    if let Some(mod_str) = parts.next() {
        let mod_parts: Vec<&str> = mod_str.split(':').collect();
        let mask: u8 = mod_parts
            .first()
            .copied()
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        modifiers = decode_modifier_mask(mask);

        if let Some(ev_type) = mod_parts.get(1).copied() {
            kind = match ev_type {
                "2" => KeyEventKind::Repeat,
                "3" => KeyEventKind::Release,
                _ => KeyEventKind::Press,
            };
        }
    }

    // ---- translate primary code to KeyCode ----
    // When SHIFT is held and the terminal reports a shifted alternate
    // codepoint, prefer it for the emitted KeyCode::Char (matching
    // crossterm's CSI-u semantics), so text-input paths that read
    // `key.code` directly see the shifted character (e.g. `*` not `8`).
    // CURRENT_ALT_KEYS still carries both variants for layout-independent
    // keybinding matching.
    let keycode = if modifiers.contains(KeyModifiers::SHIFT) {
        match shifted {
            Some(c) => u32_to_keycode(c as u32),
            None => u32_to_keycode(primary),
        }
    } else {
        u32_to_keycode(primary)
    };

    // Store alternate keys for the dispatch layer.
    CURRENT_ALT_KEYS.with(|c| c.set(AltKeys { shifted, base }));

    Ok(Some((
        Event::Key(KeyEvent::new_with_kind_and_state(
            keycode,
            modifiers,
            kind,
            KeyEventState::empty(),
        )),
        consumed,
    )))
}

/// Convert a Unicode code point (or special kitty code) to `KeyCode`.
fn u32_to_keycode(codepoint: u32) -> KeyCode {
    // Functional key codes (private-use-area codes from kitty protocol).
    if let Some(code) = translate_functional(codepoint) {
        return code;
    }
    // Regular Unicode character.
    if let Some(c) = char::from_u32(codepoint) {
        return match c {
            '\x1B' => KeyCode::Esc,
            '\r' => KeyCode::Enter,
            '\t' => KeyCode::Tab,
            '\x7F' => KeyCode::Backspace,
            _ => KeyCode::Char(c),
        };
    }
    KeyCode::Null
}

/// Map kitty-protocol functional key codes (mostly in the private-use area).
fn translate_functional(codepoint: u32) -> Option<KeyCode> {
    match codepoint {
        // Regular navigation keys (kitty private-use codepoints)
        57348 => Some(KeyCode::Insert),
        57349 => Some(KeyCode::Delete),
        57354 => Some(KeyCode::PageUp),
        57355 => Some(KeyCode::PageDown),
        57356 => Some(KeyCode::Home),
        57357 => Some(KeyCode::End),
        // Numpad
        57399..=57408 => {
            let digit = (codepoint - 57399) as u8;
            Some(KeyCode::Char((b'0' + digit) as char))
        }
        57409 => Some(KeyCode::Char('.')),
        57410 => Some(KeyCode::Char('/')),
        57411 => Some(KeyCode::Char('*')),
        57412 => Some(KeyCode::Char('-')),
        57413 => Some(KeyCode::Char('+')),
        57414 => Some(KeyCode::Enter),
        57415 => Some(KeyCode::Char('=')),
        57416 => Some(KeyCode::Char(',')),
        57417 => Some(KeyCode::Left),
        57418 => Some(KeyCode::Right),
        57419 => Some(KeyCode::Up),
        57420 => Some(KeyCode::Down),
        57421 => Some(KeyCode::PageUp),
        57422 => Some(KeyCode::PageDown),
        57423 => Some(KeyCode::Home),
        57424 => Some(KeyCode::End),
        57425 => Some(KeyCode::Insert),
        57426 => Some(KeyCode::Delete),
        57427 => Some(KeyCode::KeypadBegin),
        // Lock keys
        57358 => Some(KeyCode::CapsLock),
        57359 => Some(KeyCode::ScrollLock),
        57360 => Some(KeyCode::NumLock),
        57361 => Some(KeyCode::PrintScreen),
        57362 => Some(KeyCode::Pause),
        57363 => Some(KeyCode::Menu),
        // F13-F35
        57376..=57398 => Some(KeyCode::F((codepoint - 57376 + 13) as u8)),
        // Media keys
        57428 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Play)),
        57429 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Pause)),
        57430 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::PlayPause)),
        57431 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Reverse)),
        57432 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Stop)),
        57433 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::FastForward)),
        57434 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Rewind)),
        57435 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::TrackNext)),
        57436 => Some(KeyCode::Media(
            crossterm::event::MediaKeyCode::TrackPrevious,
        )),
        57437 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::Record)),
        57438 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::LowerVolume)),
        57439 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::RaiseVolume)),
        57440 => Some(KeyCode::Media(crossterm::event::MediaKeyCode::MuteVolume)),
        // Modifier keys
        57441 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftShift,
        )),
        57442 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftControl,
        )),
        57443 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftAlt,
        )),
        57444 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftSuper,
        )),
        57445 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftHyper,
        )),
        57446 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::LeftMeta,
        )),
        57447 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightShift,
        )),
        57448 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightControl,
        )),
        57449 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightAlt,
        )),
        57450 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightSuper,
        )),
        57451 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightHyper,
        )),
        57452 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::RightMeta,
        )),
        57453 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::IsoLevel3Shift,
        )),
        57454 => Some(KeyCode::Modifier(
            crossterm::event::ModifierKeyCode::IsoLevel5Shift,
        )),
        _ => None,
    }
}

/// Parse the kitty modifier bitmask into [`KeyModifiers`].
///
/// Kitty encodes the modifier field as `bitmask + 1` so that the value 1
/// means "no modifiers" (not absent) and 0 is never sent. Subtract 1 before
/// decoding: bits: 0=Shift, 1=Alt, 2=Ctrl, 3=Super.
pub(crate) fn decode_modifier_mask(mask: u8) -> KeyModifiers {
    let bits = mask.saturating_sub(1);
    let mut m = KeyModifiers::empty();
    if bits & 0x01 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if bits & 0x02 != 0 {
        m |= KeyModifiers::ALT;
    }
    if bits & 0x04 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    if bits & 0x08 != 0 {
        m |= KeyModifiers::SUPER;
    }
    m
}

fn parse_csi_tilde(params: &str, consumed: usize) -> io::Result<Option<(Event, usize)>> {
    // params is everything between '[' and '~'.
    // When a modifier is held, terminals send `params = "5;5"` (Ctrl+PageUp),
    // `params = "3;2"` (Shift+Delete), etc — split on ';' to get the key
    // number and optional modifier[:event_type].
    let mut parts = params.split(';');
    let num_str = parts.next().unwrap_or("0");
    let num: u16 = num_str.parse().unwrap_or(0);

    let mut modifiers = KeyModifiers::empty();
    let mut kind = KeyEventKind::Press;

    if let Some(mod_field) = parts.next() {
        let mut sub = mod_field.split(':');
        let mask: u8 = sub.next().unwrap_or("0").parse().unwrap_or(0);
        modifiers = decode_modifier_mask(mask);
        kind = match sub.next() {
            Some("2") => KeyEventKind::Repeat,
            Some("3") => KeyEventKind::Release,
            _ => KeyEventKind::Press,
        };
    }

    let code = match num {
        1 | 7 => KeyCode::Home,
        2 | 8 => KeyCode::Insert,
        3 | 9 => KeyCode::Delete,
        4 | 10 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F((num - 10) as u8), // F1-F5
        17..=21 => KeyCode::F((num - 11) as u8), // F6-F10
        23..=26 => KeyCode::F((num - 12) as u8), // F11-F14
        28..=34 => KeyCode::F((num - 13) as u8), // F15-F21
        _ => KeyCode::Null,
    };
    Ok(Some((
        Event::Key(KeyEvent::new_with_kind_and_state(
            code,
            modifiers,
            kind,
            KeyEventState::empty(),
        )),
        consumed,
    )))
}

fn parse_ss3(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    if bytes.len() < 3 {
        return Ok(None);
    }
    let code = match bytes[2] {
        b'A' => KeyCode::Up,
        b'B' => KeyCode::Down,
        b'C' => KeyCode::Right,
        b'D' => KeyCode::Left,
        b'F' => KeyCode::End,
        b'H' => KeyCode::Home,
        b'P' => KeyCode::F(1),
        b'Q' => KeyCode::F(2),
        b'R' => KeyCode::F(3),
        b'S' => KeyCode::F(4),
        _ => KeyCode::Null,
    };
    Ok(Some((key(code), 3)))
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

/// Parse a legacy CSI arrow sequence (ESC[A/B/C/D).
///
/// With DISAMBIGUATE_ESCAPE_CODES + REPORT_EVENT_TYPES the terminal sends
/// `ESC[1;<mod>[:<event_type>]X` for both press and release. Extract the event
/// type so release events can be filtered by `handle_key`.
fn csi_arrow(code: KeyCode, params: &str, consumed: usize) -> io::Result<Option<(Event, usize)>> {
    let mut modifiers = KeyModifiers::empty();
    let mut kind = KeyEventKind::Press;
    if let Some(mod_field) = params.split(';').nth(1) {
        let mut sub = mod_field.split(':');
        let mask: u8 = sub.next().unwrap_or("0").parse().unwrap_or(0);
        modifiers = decode_modifier_mask(mask);
        kind = match sub.next() {
            Some("2") => KeyEventKind::Repeat,
            Some("3") => KeyEventKind::Release,
            _ => KeyEventKind::Press,
        };
    }
    Ok(Some((
        Event::Key(KeyEvent::new_with_kind_and_state(
            code,
            modifiers,
            kind,
            KeyEventState::empty(),
        )),
        consumed,
    )))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

#[cfg(test)]
#[path = "parser_test.rs"]
mod tests;
