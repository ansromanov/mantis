//! Inline image rendering for the content pane via the Kitty graphics protocol.
//!
//! [`detect`] probes once at startup whether the terminal (Ghostty, Kitty,
//! WezTerm, Konsole) speaks the protocol. When it does and the current file is
//! an image, the loader decodes it into a [`ContentImage`] and the content-pane
//! renderer reserves a rectangle for it (`App::image_area`). After ratatui
//! paints each frame, [`render_overlay`] writes the actual bitmap escape
//! sequences to the terminal on top of that reserved region, re-placing it every
//! frame so it survives cell repaints and deleting it when an overlay covers the
//! pane or the user navigates away.

pub mod detect;
pub mod kitty;

use std::io::Write;
use std::sync::{Arc, Mutex};

use ratatui::layout::Rect;

use crate::app::App;
use kitty::ImagePlacement;

/// A decoded image ready to hand to the terminal: PNG-encoded bytes plus the
/// source pixel dimensions (used to fit the image to the pane's cell box).
#[derive(Clone)]
pub struct ContentImage {
    png: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

impl ContentImage {
    /// Decodes `bytes` (any raster format the `image` crate is built with) and
    /// returns it PNG-encoded. Returns `None` when the bytes are not a
    /// decodable image — the caller keeps the text placeholder.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let already_png = reader.format() == Some(image::ImageFormat::Png);
        let img = reader.decode().ok()?;
        let width = img.width();
        let height = img.height();
        if width == 0 || height == 0 {
            return None;
        }
        let png = if already_png {
            bytes.to_vec()
        } else {
            let mut buf = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
                .ok()?;
            buf
        };
        Some(Self {
            png: Arc::new(png),
            width,
            height,
        })
    }
}

impl std::fmt::Debug for ContentImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContentImage")
            .field("png_bytes", &self.png.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

/// What the terminal is currently displaying, so successive frames can diff.
struct Shown {
    id: u32,
    placement: ImagePlacement,
    /// Identity of the `Arc<Vec<u8>>` last transmitted, to detect a file change.
    src: usize,
}

struct RenderState {
    shown: Option<Shown>,
    next_id: u32,
}

/// Image ids cycle through this range so a stale image is never mistaken for a
/// fresh one after a rapid file switch.
const ID_LO: u32 = 1000;
const ID_HI: u32 = 1099;

static STATE: Mutex<RenderState> = Mutex::new(RenderState {
    shown: None,
    next_id: ID_LO,
});

/// Emit the escape sequence that removes every image and placement. Called on a
/// full-screen clear and on terminal restore so nothing is left orphaned.
pub fn clear_all<W: Write>(out: &mut W) {
    if !detect::kitty_graphics_supported() {
        return;
    }
    let _ = out.write_all(&kitty::delete_all());
    let _ = out.flush();
    if let Ok(mut st) = STATE.lock() {
        st.shown = None;
    }
}

/// Draw, re-place, or delete the inline image for the current frame. Call once
/// per frame, straight after `terminal.draw`.
pub fn render_overlay<W: Write>(app: &App, out: &mut W) {
    if !detect::kitty_graphics_supported() {
        return;
    }
    let Ok(mut st) = STATE.lock() else {
        return;
    };

    let desired = desired_placement(app);

    let Some((img, placement)) = desired else {
        if let Some(prev) = st.shown.take() {
            let _ = out.write_all(&kitty::delete(prev.id));
            let _ = out.flush();
        }
        return;
    };

    let src = Arc::as_ptr(&img.png) as usize;
    let unchanged = st
        .shown
        .as_ref()
        .is_some_and(|s| s.src == src && s.placement == placement);

    if unchanged {
        let id = st.shown.as_ref().unwrap().id;
        let _ = out.write_all(&kitty::place(id, placement));
        let _ = out.flush();
        return;
    }

    if let Some(prev) = st.shown.take() {
        let _ = out.write_all(&kitty::delete(prev.id));
    }
    let id = st.next_id;
    st.next_id = if st.next_id >= ID_HI {
        ID_LO
    } else {
        st.next_id + 1
    };
    let _ = out.write_all(&kitty::transmit_and_place(id, &img.png, placement));
    let _ = out.flush();
    st.shown = Some(Shown { id, placement, src });
}

/// The image and screen placement for this frame, or `None` when nothing should
/// be shown (no image loaded, pane too small, or an overlay covers the pane).
fn desired_placement(app: &App) -> Option<(&ContentImage, ImagePlacement)> {
    let img = app.content_image.as_ref()?;
    let area = app.image_area;
    if area == Rect::default() || area.width == 0 || area.height == 0 {
        return None;
    }
    if app.image_overlay_suppressed() {
        return None;
    }

    let (cols, rows) = kitty::fit(img.width, img.height, area.width, area.height);
    // Centre the image within the reserved rectangle; convert ratatui's 0-based
    // Rect to the terminal's 1-based cursor addressing.
    let col = area.x + 1 + area.width.saturating_sub(cols) / 2;
    let row = area.y + 1 + area.height.saturating_sub(rows) / 2;
    Some((
        img,
        ImagePlacement {
            cols,
            rows,
            col,
            row,
        },
    ))
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
