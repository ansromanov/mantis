//! A shared viewport scroll helper for read-only scrollable overlays.
//!
//! Viewports that render multiline content or lists (such as the help overlay
//! and the bug report diagnostic payload preview) need to track a vertical scroll
//! offset, clamp it to valid bounds based on current line counts and widget heights,
//! and process scroll commands from keyboard or mouse wheel inputs.
//!
//! This module defines the [`ScrollState`] struct which encapsulates this offset
//! state and key/mouse events, centralising bounds checking and step sizes so
//! that scrollable components behave consistently.

use serde::Serialize;

/// State helper tracking a vertical viewport scroll offset.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct ScrollState {
    /// The current 0-indexed top line of the scroll viewport.
    pub scroll: usize,
}

impl ScrollState {
    /// Creates a new `ScrollState` with an initial offset of 0.
    pub fn new() -> Self {
        ScrollState { scroll: 0 }
    }

    /// Scrolls the viewport up by `delta` lines, saturating at 0.
    pub fn scroll_up(&mut self, delta: usize) {
        self.scroll = self.scroll.saturating_sub(delta);
    }

    /// Scrolls the viewport down by `delta` lines, clamping to `max_scroll`.
    pub fn scroll_down(&mut self, delta: usize, max_scroll: usize) {
        self.scroll = (self.scroll.saturating_add(delta)).min(max_scroll);
    }

    /// Clamps the current scroll offset to a maximum value.
    pub fn clamp(&mut self, max_scroll: usize) {
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
    }
}

#[cfg(test)]
#[path = "scroll_test.rs"]
mod tests;
