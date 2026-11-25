//! Terminal mode helpers: alternate-scroll toggling and restoring the terminal on exit.

/// Toggles xterm alternate-scroll mode (DECSET 1007), which otherwise
/// translates mouse-wheel events into arrow-key presses in the alternate
/// screen. At a scroll bound (e.g. wheel-up at the first line) those
/// synthetic key presses cause visible flashing/tearing even though mantis's
/// own scroll state is a no-op. Best-effort: write errors are ignored.
///
/// mantis always restores this to *enabled* on exit rather than probing and
/// restoring whatever the terminal's mode was before mantis started (that
/// requires a synchronous DECRQM query/response round-trip). If the ambient
/// terminal had it disabled, exiting mantis will leave it enabled.
pub(crate) fn set_alternate_scroll(enabled: bool) {
    use std::io::Write;
    let sequence = if enabled {
        "\x1b[?1007h"
    } else {
        "\x1b[?1007l"
    };
    let _ = write!(std::io::stdout(), "{sequence}");
    let _ = std::io::stdout().flush();
}

/// Restores the terminal to a normal state: pops keyboard enhancement flags
/// (Unix only), disables raw mode, leaves the alternate screen, disables mouse
/// capture, restores alternate-scroll mode, and shows the cursor. Best-effort
/// and idempotent — each operation is silently ignored on error so this can be
/// called from a panic hook or from regular teardown.
#[cfg_attr(not(unix), allow(unused_imports))]
pub(crate) fn restore_terminal() {
    use std::io;

    use crossterm::cursor::Show;
    use crossterm::event::DisableMouseCapture;
    use crossterm::execute;
    use crossterm::terminal::{disable_raw_mode, LeaveAlternateScreen};

    let _ = crate::event_source::pop_keyboard_enhancement_flags();
    // Remove any inline image before leaving the alternate screen so it does not
    // linger on the primary buffer. No-op unless graphics were detected.
    crate::graphics::clear_all(&mut io::stdout());
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        LeaveAlternateScreen,
        DisableMouseCapture,
        Show
    );
    set_alternate_scroll(true);
}
