//! Kitty graphics-protocol capability detection.
//!
//! Detected once at startup (before raw mode) and cached. An environment hint
//! covers the common terminals immediately; otherwise a graphics query is sent
//! with a primary device-attributes request as a sentinel — every terminal
//! answers DA, so its reply bounds the wait even when graphics are unsupported
//! and no `_G` reply ever comes. Mirrors `theme::query_osc_11`.

use std::sync::OnceLock;

static SUPPORTED: OnceLock<bool> = OnceLock::new();

/// Whether inline image rendering should be used. False until [`detect`] runs.
pub fn kitty_graphics_supported() -> bool {
    SUPPORTED.get().copied().unwrap_or(false)
}

/// Probes terminal capability once, at startup. Safe to call more than once —
/// only the first call takes effect. Whether image preview is actually used is
/// additionally gated on the `content.image_preview` config flag at load time.
pub fn detect() {
    let supported = env_hint() || query_terminal();
    let _ = SUPPORTED.set(supported);
}

fn env_hint() -> bool {
    env_hint_from(
        std::env::var_os("KITTY_WINDOW_ID").is_some(),
        std::env::var_os("WEZTERM_EXECUTABLE").is_some(),
        std::env::var("TERM").ok().as_deref(),
        std::env::var("TERM_PROGRAM").ok().as_deref(),
    )
}

/// Pure form of [`env_hint`] for testing: does this combination of terminal
/// environment variables identify a Kitty-graphics-capable terminal?
fn env_hint_from(
    kitty_window_id: bool,
    wezterm_executable: bool,
    term: Option<&str>,
    term_program: Option<&str>,
) -> bool {
    if kitty_window_id || wezterm_executable {
        return true;
    }
    if term.is_some_and(|t| t.contains("kitty") || t.contains("ghostty")) {
        return true;
    }
    term_program.is_some_and(|t| {
        let t = t.to_ascii_lowercase();
        t == "ghostty" || t == "wezterm"
    })
}

#[cfg(unix)]
fn query_terminal() -> bool {
    use std::io::{IsTerminal, Write};
    use std::time::{Duration, Instant};

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return false;
    }

    let mut stdout = std::io::stdout();
    // 1x1 RGB query (a=q) followed by primary device-attributes (\x1b[c).
    if stdout
        .write_all(b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAAAAAAAAAA\x1b\\\x1b[c")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        return false;
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(200);
    let mut buf = Vec::new();
    let mut poll_fd = libc::pollfd {
        fd: 0,
        events: libc::POLLIN,
        revents: 0,
    };

    while start.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(start.elapsed())
            .unwrap_or(Duration::ZERO);
        let ret = unsafe { libc::poll(&mut poll_fd, 1, remaining.as_millis() as libc::c_int) };
        if ret > 0 && (poll_fd.revents & libc::POLLIN) != 0 {
            let mut byte = 0u8;
            let n = unsafe { libc::read(0, &mut byte as *mut u8 as *mut libc::c_void, 1) };
            if n <= 0 {
                break;
            }
            buf.push(byte);
            // The DA reply ends with 'c'; once it arrives, no _G reply is coming.
            if byte == b'c' && buf.contains(&0x1b) {
                break;
            }
        } else {
            break;
        }
    }

    let resp = String::from_utf8_lossy(&buf);
    resp.contains("_G") && resp.contains(";OK")
}

#[cfg(not(unix))]
fn query_terminal() -> bool {
    false
}

#[cfg(test)]
#[path = "detect_test.rs"]
mod tests;
