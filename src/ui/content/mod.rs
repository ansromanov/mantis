//! Content-pane rendering, split into focused submodules.
//!
//! This is the thin module root for the right-hand content/diff panel. It wires
//! up the submodules - `draw` (the main dispatch across file, markdown, virtual,
//! and fallback views), `diff` (side-by-side diff layout), `scrollbar` (the
//! transient scrollbar overlay), `search` (in-file match highlighting), and
//! `selection` (text-selection highlighting) - and re-exports `draw_content` for
//! the UI orchestrator to call. Keep rendering logic in the submodules; this
//! file only declares them and exposes the single entry point, with co-located
//! tests in `content_test.rs`.

mod diff;
mod draw;
mod draw_text;
mod scrollbar;
mod search;
mod selection;

pub(super) use draw::draw_content;

#[cfg(test)]
#[path = "content_test.rs"]
mod tests;
