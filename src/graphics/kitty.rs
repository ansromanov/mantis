//! Kitty graphics-protocol escape-sequence builders.
//!
//! Pure functions, no I/O. The content-pane overlay
//! ([`crate::graphics::render_overlay`]) writes the bytes these return straight
//! to stdout after ratatui has painted a frame.
//!
//! `transmit_and_place` sends the PNG bytes and displays them; `place`
//! re-displays an already-transmitted image (cheap enough to send every frame so
//! the image survives a ratatui repaint of the region); `delete` / `delete_all`
//! remove images.

/// Max base64 payload per `\x1b_G` chunk, per the protocol's 4096-byte guidance.
const CHUNK: usize = 4096;

/// Fixed placement id — reusing one id means a repeated `put` replaces the old
/// placement instead of stacking a new one on top.
const PLACEMENT: u32 = 1;

/// Target cell box and screen origin for an image placement. Coordinates are
/// 1-based (terminal cursor addressing). `cols`/`rows` is the character-cell
/// rectangle the terminal scales the image into, preserving aspect ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePlacement {
    pub cols: u16,
    pub rows: u16,
    pub col: u16,
    pub row: u16,
}

/// Chooses a cell box no larger than `max_cols` x `max_rows` that preserves the
/// image's aspect ratio, assuming a terminal cell is roughly twice as tall as it
/// is wide. Always returns at least 1x1.
pub fn fit(img_w: u32, img_h: u32, max_cols: u16, max_rows: u16) -> (u16, u16) {
    let max_cols = max_cols.max(1);
    let max_rows = max_rows.max(1);
    if img_w == 0 || img_h == 0 {
        return (max_cols, max_rows);
    }
    // Aspect in cell units: one cell ~= 1 wide x 2 tall pixels.
    let img_w = img_w as f64;
    let img_h = img_h as f64 / 2.0;

    let scale = (max_cols as f64 / img_w).min(max_rows as f64 / img_h);
    let cols = (img_w * scale).round().clamp(1.0, max_cols as f64) as u16;
    let rows = (img_h * scale).round().clamp(1.0, max_rows as f64) as u16;
    (cols, rows)
}

fn cursor_home(row: u16, col: u16) -> Vec<u8> {
    format!("\x1b[{row};{col}H").into_bytes()
}

/// Transmit `png_bytes` under image id `id` and display it in the `p.cols` x
/// `p.rows` cell box with its top-left at `(p.row, p.col)`, leaving the cursor
/// where it was (`C=1`).
pub fn transmit_and_place(id: u32, png_bytes: &[u8], p: ImagePlacement) -> Vec<u8> {
    let mut out = cursor_home(p.row, p.col);
    let b64 = base64_encode(png_bytes);
    let payload = b64.as_bytes();
    if payload.is_empty() {
        return out;
    }

    let mut chunks = payload.chunks(CHUNK).peekable();
    let mut first = true;
    while let Some(chunk) = chunks.next() {
        let more = u8::from(chunks.peek().is_some());
        out.extend_from_slice(b"\x1b_G");
        if first {
            out.extend_from_slice(
                format!(
                    "a=T,f=100,i={id},p={PLACEMENT},c={},r={},C=1,q=2,m={more}",
                    p.cols, p.rows
                )
                .as_bytes(),
            );
            first = false;
        } else {
            out.extend_from_slice(format!("m={more}").as_bytes());
        }
        out.push(b';');
        out.extend_from_slice(chunk);
        out.extend_from_slice(b"\x1b\\");
    }
    out
}

/// Re-display already-transmitted image `id` at `(p.row, p.col)` in a `p.cols` x
/// `p.rows` cell box, reusing the fixed placement id so no placement stacks up.
pub fn place(id: u32, p: ImagePlacement) -> Vec<u8> {
    let mut out = cursor_home(p.row, p.col);
    out.extend_from_slice(
        format!(
            "\x1b_Ga=p,i={id},p={PLACEMENT},c={},r={},C=1,q=2\x1b\\",
            p.cols, p.rows
        )
        .as_bytes(),
    );
    out
}

/// Delete image `id` and free its data.
pub fn delete(id: u32) -> Vec<u8> {
    format!("\x1b_Ga=d,d=i,i={id},q=2\x1b\\").into_bytes()
}

/// Delete every image and placement (used on full-screen clear and on exit).
pub fn delete_all() -> Vec<u8> {
    b"\x1b_Ga=d,d=A,q=2\x1b\\".to_vec()
}

/// Standard base64 with `=` padding.
fn base64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
#[path = "kitty_test.rs"]
mod tests;
