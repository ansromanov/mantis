//! Raw terminal event source with kitty keyboard protocol support.
//!
//! Reads bytes directly from stdin via `libc::poll`/`read` so it can parse
//! CSI-u sequences and extract the *alternate keycode* (the US-layout physical
//! key) that crossterm discards. The extracted base key is stored in a
//! `thread_local!` cell so [`KeyBinding::matches`](crate::config::KeyBinding)
//! can use it for layout-independent matching.

use std::cell::Cell;
use std::io;

use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};

// ---------------------------------------------------------------------------
// Thread-local base key for the current event
// ---------------------------------------------------------------------------

thread_local! {
    /// Physical (US-layout) key for the event currently being dispatched, if
    /// the terminal provided alternate-keycode information via the kitty
    /// keyboard protocol.
    pub static CURRENT_BASE_KEY: Cell<Option<KeyCode>> = const { Cell::new(None) };
}

// ---------------------------------------------------------------------------
// Keyboard enhancement flag management
// ---------------------------------------------------------------------------

/// Attempt to enable the kitty keyboard protocol on the terminal.
///
/// Returns `true` when the terminal supports it and the flags were
/// successfully pushed. The caller **must** call
/// [`pop_keyboard_enhancement_flags`] on teardown.
pub fn push_keyboard_enhancement_flags() -> io::Result<bool> {
    let supported = crossterm::terminal::supports_keyboard_enhancement()?;
    if supported {
        use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};
        use crossterm::execute;
        execute!(
            io::stdout(),
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
            )
        )?;
    }
    Ok(supported)
}

/// Pop the keyboard enhancement flags pushed earlier.
pub fn pop_keyboard_enhancement_flags() -> io::Result<()> {
    use crossterm::event::PopKeyboardEnhancementFlags;
    use crossterm::execute;
    execute!(io::stdout(), PopKeyboardEnhancementFlags)
}

// ---------------------------------------------------------------------------
// Raw event source
// ---------------------------------------------------------------------------

/// An [`EventSource`](crate::EventSource) that parses raw terminal bytes so it
/// can extract kitty-protocol alternate keycodes.
pub struct RawEventSource {
    buf: Vec<u8>,
    pos: usize,
}

impl Default for RawEventSource {
    fn default() -> Self {
        Self::new()
    }
}

impl RawEventSource {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
            pos: 0,
        }
    }

    /// Fill the internal buffer from stdin (non-blocking after a 16 ms poll).
    fn fill(&mut self) -> io::Result<()> {
        // Discard already-consumed bytes.
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }

        // Poll for data on fd 0 with a 16 ms timeout.
        let mut pfd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        let ret = unsafe { libc::poll(&mut pfd, 1, 16) };
        if ret <= 0 {
            return Ok(()); // timeout or error
        }

        let mut tmp = [0u8; 4096];
        loop {
            let n = unsafe { libc::read(0, tmp.as_mut_ptr() as *mut libc::c_void, tmp.len()) };
            if n <= 0 {
                break;
            }
            self.buf.extend_from_slice(&tmp[..n as usize]);
            if (n as usize) < tmp.len() {
                break; // no more bytes available right now
            }
        }
        Ok(())
    }

    /// Parse and return the next event from the buffer, if one is available.
    pub fn next_raw_event(&mut self) -> io::Result<Option<Event>> {
        // No event carries a stale base key from a previous call.
        CURRENT_BASE_KEY.with(|cell| cell.set(None));
        self.fill()?;
        if self.pos >= self.buf.len() {
            return Ok(None);
        }

        let remaining = &self.buf[self.pos..];
        if remaining.is_empty() {
            return Ok(None);
        }

        match parse_event(remaining) {
            Ok(Some((event, consumed))) => {
                self.pos += consumed;
                Ok(Some(event))
            }
            Ok(None) => {
                // Incomplete sequence – skip it and wait for more data.
                self.buf.clear();
                self.pos = 0;
                Ok(None)
            }
            Err(e) => {
                // Unparseable: skip one byte and try again.
                self.pos += 1;
                Err(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Low-level parser
// ---------------------------------------------------------------------------

/// Try to parse a single event from the front of `bytes`.
///
/// Returns `Ok(Some((event, consumed)))` on success, `Ok(None)` for an
/// incomplete sequence (need more data), or `Err` for an unparseable byte.
fn parse_event(bytes: &[u8]) -> io::Result<Option<(Event, usize)>> {
    if bytes.is_empty() {
        return Ok(None);
    }

    match bytes[0] {
        0x1B => parse_escape(bytes),
        // Ctrl+@ through Ctrl+_ (and DEL / Ctrl+?)
        // 0x09 = Tab, 0x0D = CR, 0x1B = ESC — handled individually below.
        0x00..=0x06 | 0x08 | 0x0A | 0x0C | 0x0E..=0x1A | 0x1C..=0x1F => {
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
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unhandled byte 0x{:02X}", bytes[0]),
        )),
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

// ---------------------------------------------------------------------------
// CSI parser (ESC [ ... )
// ---------------------------------------------------------------------------

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
        b'A' => Ok(Some((key(KeyCode::Up), consumed))),
        b'B' => Ok(Some((key(KeyCode::Down), consumed))),
        b'C' => Ok(Some((key(KeyCode::Right), consumed))),
        b'D' => Ok(Some((key(KeyCode::Left), consumed))),
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

// ---------------------------------------------------------------------------
// CSI-u / kitty key event parser
// ---------------------------------------------------------------------------

/// Parse a CSI-u key event.
///
/// Format: `keycode[:alternate][; modifiers[:event_type]]`
///
/// We extract the **first alternate keycode** as the US-layout physical key
/// and store it in [`CURRENT_BASE_KEY`].
fn parse_csi_u(params: &str, consumed: usize) -> io::Result<Option<(Event, usize)>> {
    let mut parts = params.split(';');

    // ---- key codes (primary[:alternate[:...]]) ----
    let codes_str = parts.next().unwrap_or("");
    let mut codes = codes_str.split(':');

    let primary_str = codes.next().unwrap_or("0");
    let primary: u32 = primary_str
        .parse()
        .map_err(|_| invalid("bad primary keycode"))?;

    // First alternate keycode = US-layout physical key (if present).
    let base_key = codes
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .and_then(char::from_u32)
        .map(KeyCode::Char);

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
    let keycode = u32_to_keycode(primary);

    // Store base key for the dispatch layer.
    CURRENT_BASE_KEY.with(|cell| cell.set(base_key));

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
/// Bits: 1=Shift, 2=Alt, 4=Ctrl, 8=Super, 16=Hyper, 32=Meta.
fn decode_modifier_mask(mask: u8) -> KeyModifiers {
    let mut m = KeyModifiers::empty();
    if mask & 0x01 != 0 {
        m |= KeyModifiers::SHIFT;
    }
    if mask & 0x02 != 0 {
        m |= KeyModifiers::ALT;
    }
    if mask & 0x04 != 0 {
        m |= KeyModifiers::CONTROL;
    }
    if mask & 0x08 != 0 {
        m |= KeyModifiers::SUPER;
    }
    m
}

// ---------------------------------------------------------------------------
// CSI ~ (function-key) parser
// ---------------------------------------------------------------------------

fn parse_csi_tilde(params: &str, consumed: usize) -> io::Result<Option<(Event, usize)>> {
    // params is everything between '[' and '~'
    let num: u16 = params.parse().unwrap_or(0);
    let code = match num {
        1 | 7 => KeyCode::Home,
        2 | 8 => KeyCode::Insert,
        3 | 9 => KeyCode::Delete,
        4 | 10 => KeyCode::End,
        5 => KeyCode::PageUp,
        6 => KeyCode::PageDown,
        11..=15 => KeyCode::F((num - 11 + 1) as u8),
        17..=21 => KeyCode::F((num - 17 + 1) as u8),
        23..=26 => KeyCode::F((num - 23 + 1) as u8),
        28..=34 => KeyCode::F((num - 28 + 1) as u8),
        _ => KeyCode::Null,
    };
    Ok(Some((key(code), consumed)))
}

// ---------------------------------------------------------------------------
// SS3 parser (ESC O ... )
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// SGR mouse parser
// ---------------------------------------------------------------------------

/// Parse an SGR-encoded mouse event.
///
/// Format: `code;col;row` (parameters between '[' and the final M/m).
fn parse_sgr_mouse(
    params: &str,
    consumed: usize,
    _default_kind: MouseEventKind,
) -> io::Result<Option<(Event, usize)>> {
    let mut parts = params.split(';');
    let cb_str = parts.next().unwrap_or("0");
    let col_str = parts.next().unwrap_or("1");
    let row_str = parts.next().unwrap_or("1");

    let cb: u16 = cb_str.parse().unwrap_or(0);
    let col: u16 = col_str.parse().unwrap_or(0);
    let row: u16 = row_str.parse().unwrap_or(0);

    // Button 0-2: press; 0x20: motion (drag); 0x40: scroll (wheel).
    let kind = if cb & 0x40 != 0 {
        // Scroll event
        if cb & 0x01 != 0 {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        }
    } else if cb & 0x20 != 0 {
        MouseEventKind::Moved
    } else if cb & 0x03 == 3 {
        // No button (release in X10 mode)
        MouseEventKind::Up(decode_mouse_button(cb))
    } else if cb & 0x80 != 0 {
        MouseEventKind::Up(decode_mouse_button(cb))
    } else {
        MouseEventKind::Down(decode_mouse_button(cb))
    };

    Ok(Some((
        Event::Mouse(MouseEvent {
            kind,
            column: col.saturating_sub(1),
            row: row.saturating_sub(1),
            modifiers: KeyModifiers::empty(),
        }),
        consumed,
    )))
}

fn decode_mouse_button(cb: u16) -> MouseButton {
    match cb & 0x03 {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        _ => MouseButton::Left,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}
