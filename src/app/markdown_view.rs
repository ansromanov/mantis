//! Synchronous rendered-markdown view for the content pane.
//!
//! When a markdown file finishes loading, [`App::install_native_markdown`]
//! renders it with [`crate::markdown`] and installs the result as the
//! content-pane takeover (the same `rendered_content` storage external plugins
//! used) *before the first frame is drawn*. There is no raw-then-rendered swap,
//! so the gutter, width and scroll position stay put. Rendering is skipped for
//! very large files, which fall back to the raw view.

use std::path::Path;

use super::App;
use crate::markdown::{render_to_ansi, MarkdownColors};

/// Files above this size are shown raw instead of rendered.
const MAX_RENDER_BYTES: u64 = 2 * 1024 * 1024;

impl App {
    /// Renders `path` as markdown and makes it the active content view.
    /// Leaves the raw view in place when the file is too large or unreadable.
    pub(super) fn install_native_markdown(&mut self, path: &Path) {
        let too_large = std::fs::metadata(path).map_or(true, |m| m.len() > MAX_RENDER_BYTES);
        if too_large {
            return;
        }
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let colors = MarkdownColors::from_theme(&self.theme);
        let rendered: Vec<Vec<(ratatui::style::Style, String)>> = render_to_ansi(&source, &colors)
            .iter()
            .map(|line| crate::ansi::parse_ansi_line(line))
            .collect();
        let text: Vec<String> = rendered
            .iter()
            .map(|spans| spans.iter().map(|(_, value)| value.as_str()).collect())
            .collect();
        self.rendered_text.insert(path.to_path_buf(), text);
        self.rendered_content.insert(path.to_path_buf(), rendered);
        self.rendered_active_path = Some(path.to_path_buf());
        self.rendered_active = true;
        self.content_revision += 1;
        self.clamp_content_scroll();
    }

    /// Drops the rendered view of `path` so the raw file is shown.
    pub(super) fn clear_native_markdown(&mut self, path: &Path) {
        self.rendered_content.remove(path);
        self.rendered_text.remove(path);
        self.rendered_active = false;
        self.rendered_active_path = None;
    }
}

#[cfg(test)]
#[path = "markdown_view_test.rs"]
mod tests;
