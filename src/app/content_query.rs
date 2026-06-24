//! Read-only queries over the active content source for `App`.
//!
//! The content pane can be backed by several sources depending on what is open:
//! a memory-mapped [`VirtualFile`](crate::virtual_file::VirtualFile), an
//! in-memory `Vec<String>`, pretty-printed JSON, or rendered markdown spans.
//! These helpers (`line_count`, `line_text`, and friends) hide that branching
//! behind a single interface so the rest of the app can ask "how many lines?" or
//! "what is line N?" without knowing which source is active. They never mutate
//! state; callers that need scrolling or navigation build on top of these read
//! accessors.

use super::App;

impl App {
    /// Returns the total number of lines in the current content source
    /// (plugin content, virtual file, raw content, JSON pretty, or markdown).
    pub fn line_count(&self) -> usize {
        if let Some(path) = &self.current_file {
            if let Some(lines) = self.plugin_content.get(path) {
                return lines.len();
            }
        }
        if self.is_markdown && !self.show_raw_markdown && !self.markdown_lines.is_empty() {
            self.markdown_lines.len()
        } else if self.is_json && self.show_pretty_json && !self.json_pretty_lines.is_empty() {
            self.json_pretty_lines.len()
        } else if let Some(vf) = &self.virtual_file {
            vf.line_count()
        } else {
            self.content.len()
        }
    }

    /// Returns the text of the 0-indexed line, consulting the active content
    /// source: plugin content, pretty JSON, virtual file, or raw content vec.
    pub fn line_text(&self, index: usize) -> Option<&str> {
        if let Some(path) = &self.current_file {
            if let Some(lines) = self.plugin_content_text.get(path) {
                return lines.get(index).map(|s| s.as_str());
            }
        }
        if self.is_json && self.show_pretty_json && !self.json_pretty_text.is_empty() {
            self.json_pretty_text.get(index).map(|s| s.as_str())
        } else if let Some(vf) = &self.virtual_file {
            vf.line_text(index)
        } else {
            self.content.get(index).map(|s| s.as_str())
        }
    }

    /// Returns the display width of line `index` in terminal columns.
    pub fn line_width(&self, index: usize) -> Option<usize> {
        if let Some(vf) = &self.virtual_file {
            vf.line_width(index)
        } else {
            self.line_text(index)
                .map(unicode_width::UnicodeWidthStr::width)
        }
    }

    /// Syntax-highlights a slice of lines for the visible window.
    pub fn highlight_lines(
        &self,
        path: &std::path::Path,
        lines: &[&str],
    ) -> Vec<Vec<(ratatui::style::Style, String)>> {
        self.highlighter.highlight_range(path, lines)
    }

    /// Whether the diff should currently render in the side-by-side layout: the
    /// toggle is on, a diff is loaded, and the content pane is wide enough.
    pub fn diff_sbs_active(&self) -> bool {
        self.is_diff
            && self.diff_side_by_side
            && !self.diff_rows.is_empty()
            && self.content_area.width >= crate::diff::MIN_SIDE_BY_SIDE_WIDTH
    }

    /// Returns the number of **display** lines after folding. Equals
    /// `line_count()` when no folds are active.
    pub fn display_line_count(&self) -> usize {
        if self.diff_sbs_active() {
            self.diff_rows.len()
        } else if self.fold_display_map.is_empty() {
            self.line_count()
        } else {
            self.fold_display_map.len()
        }
    }

    /// Maps a display-space line index to a physical file line index.
    pub fn display_to_physical(&self, display: usize) -> usize {
        if self.fold_display_map.is_empty() {
            display
        } else {
            self.fold_display_map
                .get(display)
                .copied()
                .unwrap_or(display)
        }
    }

    /// Converts a physical line index to a display line index.
    /// When folding is inactive this is identity; when active it finds the
    /// position of `physical` in the display map (first visible line ≥ physical
    /// when the line itself is hidden inside a fold).
    pub fn physical_to_display(&self, physical: usize) -> usize {
        if self.fold_display_map.is_empty() {
            return physical;
        }
        // Find the first display line whose physical index is >= physical.
        self.fold_display_map
            .iter()
            .position(|&p| p >= physical)
            .unwrap_or(self.fold_display_map.len().saturating_sub(1))
    }
}

#[cfg(test)]
#[path = "content_query_test.rs"]
mod content_query_test;
