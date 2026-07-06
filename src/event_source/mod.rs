//! Input event polling and raw terminal source management.
//!
//! This module coordinates terminal input processing by buffering raw bytes from
//! standard input, managing keyboard enhancement protocol states (e.g. Kitty
//! keyboard protocol support), and providing access to thread-local alternate
//! key codes. It acts as the orchestrator that feeds incoming terminal bytes to
//! specialized parsers to produce structured crossterm events.
//!
//! # Why a custom parser exists
//!
//! The kitty keyboard protocol sends extra fields in CSI-u sequences that
//! crossterm 0.28.x strips during parsing: the **shifted alternate** character
//! (e.g. `*` for a US keyboard's Shift+8) and the **base-layout** character
//! (the US-physical key that maps to the pressed key on non-Latin layouts).
//! These fields are needed for layout-independent keybinding matching — without
//! them, keybindings like `Ctrl+b` fail on Russian or other non-Latin keyboard
//! layouts. Crossterm's `KeyEvent` did not expose `alternate` / `base_layout_key`
//! until 0.29+, so mantis maintains its own byte-level parser for the kitty
//! path and falls back to crossterm's `event::read()` when kitty is unavailable.
//!
//! The custom parser duplicates crossterm's internal ESC/CSI parsing for
//! everything *except* CSI-u alternate-key extraction. Every fix to crossterm's
//! parser that we also need must be ported here. This is the principal
//! maintenance cost.
//!
//! When the crossterm dependency is upgraded to a version whose `KeyEvent`
//! exposes alternate key fields AND the version's own parser preserves them
//! through `event::read()`, the custom parser in `parser::` should be deleted
//! and replaced with the same `CrosstermEvents` path used on non-Unix.
//!
//! Public items:
//! - [`AltKeys`]: Shifted and physical base key codepoint alternatives.
//! - [`CURRENT_ALT_KEYS`]: Thread-local alternate keys for the current event.
//! - [`RawEventSource`]: The main buffered, non-blocking event input source
//!   (Unix only). Reads fd 0 by default (`new`), or a reopened `/dev/tty` in
//!   pager mode (`for_tty`) so keyboard input keeps working while fd 0 is
//!   piped content. On Windows, pager mode reopens `CONIN$` instead (see
//!   [`crate::redirect_stdin_to_console`]).
//! - [`push_keyboard_enhancement_flags`]: Enable Kitty enhancement support.
//! - [`pop_keyboard_enhancement_flags`]: Restore standard terminal behavior.

use std::cell::Cell;
use std::io;

use crossterm::event::Event;

pub(crate) mod mouse;
pub mod parser;

// ---------------------------------------------------------------------------
// Thread-local alternate keys for the current event
// ---------------------------------------------------------------------------

/// Kitty alternate key codes for the current event: the shifted key and the
/// US-physical (base-layout) key. Both `None` when the terminal didn't report
/// them.
#[derive(Clone, Copy, Default)]
pub struct AltKeys {
    pub shifted: Option<char>,
    pub base: Option<char>,
}

thread_local! {
    /// Alternate keycodes for the event currently being dispatched, if the
    /// terminal provided them via the kitty keyboard protocol.
    pub static CURRENT_ALT_KEYS: Cell<AltKeys> = const {
        Cell::new(AltKeys { shifted: None, base: None })
    };
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
                    // Without this, text-producing keys are sent as plain UTF-8
                    // on press (e.g. Russian `и` for physical `b`), so the
                    // base-layout alternate key never reaches the dispatcher and
                    // layout-independent letter bindings fail. Forcing every key
                    // to a CSI-u escape makes the base-layout field available on
                    // press. See parse_csi_u / KeyBinding::matches.
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
// Raw event source (Unix only)
// ---------------------------------------------------------------------------

/// Abstraction for poll+read from a file descriptor, so the
/// read-until-drained loop in [`RawEventSource::fill_with`] can be
/// unit-tested without a real stdin.
#[cfg(unix)]
trait PollReader {
    /// Returns `true` when data is available for reading.
    fn poll(&mut self, timeout_ms: libc::c_int) -> io::Result<bool>;
    /// Reads into `buf`, returns bytes read (0 = EOF).
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

/// Real [`PollReader`] that reads from a given raw file descriptor: fd 0
/// (stdin) normally, or a reopened `/dev/tty` in pager mode (see
/// [`RawEventSource::for_tty`]) where fd 0 is consumed by piped content.
#[cfg(unix)]
struct StdinReader {
    fd: libc::c_int,
}

#[cfg(unix)]
impl PollReader for StdinReader {
    fn poll(&mut self, timeout_ms: libc::c_int) -> io::Result<bool> {
        loop {
            let mut pfd = libc::pollfd {
                fd: self.fd,
                events: libc::POLLIN,
                revents: 0,
            };
            let ret = unsafe { libc::poll(&mut pfd, 1, timeout_ms) };
            if ret < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            return Ok(ret > 0 && pfd.revents & libc::POLLIN != 0);
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let n =
                unsafe { libc::read(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(err);
            }
            return Ok(n as usize);
        }
    }
}

/// An [`EventSource`](crate::EventSource) that parses raw terminal bytes so it
/// can extract kitty-protocol alternate keycodes.
#[cfg(unix)]
pub struct RawEventSource {
    buf: Vec<u8>,
    pos: usize,
    fd: libc::c_int,
    /// Keeps a reopened `/dev/tty` alive for the source's lifetime when
    /// reading from the terminal directly rather than fd 0 (see
    /// [`RawEventSource::for_tty`]). `None` when reading from stdin.
    _tty_file: Option<std::fs::File>,
}

#[cfg(unix)]
impl Default for RawEventSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(unix)]
impl RawEventSource {
    /// Reads from fd 0 (stdin), the normal case.
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(4096),
            pos: 0,
            fd: libc::STDIN_FILENO,
            _tty_file: None,
        }
    }

    /// Reads from a reopened `/dev/tty` instead of stdin. Used in pager mode,
    /// where fd 0 is consumed by piped content (`git diff | mantis`) so
    /// keyboard input must come from the controlling terminal instead — the
    /// standard pager trick (`less` does the same). Falls back to stdin if
    /// `/dev/tty` cannot be opened (e.g. no controlling terminal).
    pub fn for_tty() -> Self {
        Self::from_tty_opener(|| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty")
        })
    }

    /// Testable core of `for_tty`: takes the `/dev/tty` open as an injectable
    /// closure so the fallback path can be exercised without a real terminal.
    fn from_tty_opener(open: impl FnOnce() -> io::Result<std::fs::File>) -> Self {
        use std::os::unix::io::AsRawFd;
        match open() {
            Ok(file) => Self {
                buf: Vec::with_capacity(4096),
                pos: 0,
                fd: file.as_raw_fd(),
                _tty_file: Some(file),
            },
            Err(_) => Self::new(),
        }
    }

    /// Fill the internal buffer from the source fd with the given poll
    /// timeout (ms). Use `timeout_ms = 16` for the blocking next-event path
    /// and `timeout_ms = 0` for the non-blocking try-next-event path.
    fn fill(&mut self, timeout_ms: libc::c_int) -> io::Result<()> {
        // Discard already-consumed bytes.
        if self.pos > 0 {
            self.buf.drain(..self.pos);
            self.pos = 0;
        }
        let mut reader = StdinReader { fd: self.fd };
        self.fill_with(&mut reader, timeout_ms)
    }

    /// Read from `reader` until no more data is available, using a
    /// `poll(0)` check before every subsequent read to avoid blocking
    /// when the first read returned exactly the buffer size
    /// (fixes #456).
    fn fill_with(
        &mut self,
        reader: &mut dyn PollReader,
        timeout_ms: libc::c_int,
    ) -> io::Result<()> {
        if !reader.poll(timeout_ms)? {
            return Ok(());
        }

        let mut tmp = [0u8; 4096];
        loop {
            let n = reader.read(&mut tmp)?;
            if n == 0 {
                break;
            }
            self.buf.extend_from_slice(&tmp[..n]);
            if n < tmp.len() {
                break;
            }
            // Poll with zero timeout to check if more data is available
            // without blocking (fixes #456: exact-4096-burst freeze).
            if !reader.poll(0)? {
                break;
            }
        }
        Ok(())
    }

    fn parse_next(&mut self) -> io::Result<Option<Event>> {
        if self.pos >= self.buf.len() {
            return Ok(None);
        }

        let remaining = &self.buf[self.pos..];
        if remaining.is_empty() {
            return Ok(None);
        }

        match parser::parse_event(remaining) {
            Ok(Some((event, consumed))) => {
                self.pos += consumed;
                Ok(Some(event))
            }
            Ok(None) => {
                // Incomplete sequence — leave bytes in buffer for next fill().
                Ok(None)
            }
            Err(_) => {
                // Unparseable byte: skip it and signal no event (don't crash).
                self.pos += 1;
                Ok(None)
            }
        }
    }

    /// Parse and return the next event from the buffer, blocking briefly (16 ms)
    /// for data when the buffer is empty.
    pub fn next_raw_event(&mut self) -> io::Result<Option<Event>> {
        CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
        self.fill(16)?;
        self.parse_next()
    }

    /// Try to return an already-buffered event without waiting.
    /// Returns `None` when no event is immediately available.
    pub fn try_next_raw_event(&mut self) -> io::Result<Option<Event>> {
        CURRENT_ALT_KEYS.with(|c| c.set(AltKeys::default()));
        self.fill(0)?;
        self.parse_next()
    }
}

#[cfg(all(test, unix))]
#[path = "mod_test.rs"]
mod tests;
