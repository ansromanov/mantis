//! Light/dark terminal background detection (`COLORFGBG`, OSC 11).

use std::io::IsTerminal;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

static DETECTED_BACKGROUND: OnceLock<ThemeMode> = OnceLock::new();

pub fn get_detected_background() -> Option<ThemeMode> {
    DETECTED_BACKGROUND.get().copied()
}

pub fn detect_terminal_background() {
    let mode = get_colorfgbg_background()
        .or_else(query_osc_11)
        .unwrap_or(ThemeMode::Dark);
    let _ = DETECTED_BACKGROUND.set(mode);
}

pub(super) fn get_colorfgbg_background() -> Option<ThemeMode> {
    if let Ok(var) = std::env::var("COLORFGBG") {
        let parts: Vec<&str> = var.split(';').collect();
        if parts.len() >= 2 {
            if let Ok(bg_idx) = parts[1].parse::<u32>() {
                if bg_idx == 7 || (9..=15).contains(&bg_idx) || bg_idx >= 244 {
                    return Some(ThemeMode::Light);
                } else {
                    return Some(ThemeMode::Dark);
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn query_osc_11() -> Option<ThemeMode> {
    use std::io::Write;
    use std::time::{Duration, Instant};

    // Stdin/stdout must be terminals
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return None;
    }

    // Write OSC 11 query
    let mut stdout = std::io::stdout();
    write!(stdout, "\x1b]11;?\x07").ok()?;
    stdout.flush().ok()?;

    // Read with timeout
    let start_time = Instant::now();
    let timeout = Duration::from_millis(50);
    let mut buffer = Vec::new();

    let mut poll_fd = libc::pollfd {
        fd: 0, // stdin
        events: libc::POLLIN,
        revents: 0,
    };

    while start_time.elapsed() < timeout {
        let remaining = timeout
            .checked_sub(start_time.elapsed())
            .unwrap_or(Duration::ZERO);
        let ret = unsafe { libc::poll(&mut poll_fd, 1, remaining.as_millis() as libc::c_int) };
        if ret > 0 && (poll_fd.revents & libc::POLLIN) != 0 {
            let mut byte = 0u8;
            let n = unsafe { libc::read(0, &mut byte as *mut u8 as *mut libc::c_void, 1) };
            if n > 0 {
                buffer.push(byte);
                if byte == 0x07 {
                    break;
                }
                if buffer.len() >= 2
                    && buffer[buffer.len() - 2] == 0x1b
                    && buffer[buffer.len() - 1] == b'\\'
                {
                    break;
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    let resp = String::from_utf8_lossy(&buffer);
    let (r, g, b) = parse_osc_response(&resp)?;
    let y = (2126 * r as u32 + 7152 * g as u32 + 722 * b as u32) / 10000;
    if y > 128 {
        Some(ThemeMode::Light)
    } else {
        Some(ThemeMode::Dark)
    }
}

#[cfg(not(unix))]
fn query_osc_11() -> Option<ThemeMode> {
    None
}

pub(super) fn parse_osc_response(resp: &str) -> Option<(u8, u8, u8)> {
    let rgb_start = resp.find("rgb:")?;
    let content = &resp[rgb_start + 4..];
    let mut parts = Vec::new();
    let mut current = String::new();
    for c in content.chars() {
        if c.is_ascii_hexdigit() {
            current.push(c);
        } else if c == '/' {
            parts.push(current);
            current = String::new();
        } else {
            break;
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    if parts.len() != 3 {
        return None;
    }
    let mut rgb = [0u8; 3];
    for i in 0..3 {
        let part = &parts[i];
        if part.len() < 2 {
            return None;
        }
        rgb[i] = u8::from_str_radix(&part[0..2], 16).ok()?;
    }
    Some((rgb[0], rgb[1], rgb[2]))
}
