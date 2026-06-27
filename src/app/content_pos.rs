//! Content-pane geometry and scroll math for `App`.
//!
//! These helpers translate between the logical content (lines, columns, folds,
//! line-number gutter) and the on-screen viewport: the maximum legal scroll
//! offset, the width of the line-number/fold gutter prefix, and clamping of the
//! cursor/scroll so the last line never scrolls past the bottom edge. They are
//! pure functions of the current `App` state and the recorded `content_area`
//! rectangle, so they read geometry captured during the last render rather than
//! computing layout themselves. Mouse and keyboard handlers rely on this module
//! to keep click coordinates and scrolling consistent.

use std::num::NonZeroUsize;

use unicode_width::UnicodeWidthStr;

use super::App;

/// Number of lines to scroll per mouse wheel tick or drag-edge auto-scroll.
pub const WHEEL_STEP: usize = 3;

/// Extract the selected text from styled span lines (plugin content or rendered
/// markdown share the same `Vec<Vec<(Style, String)>>` shape). `start`/`end` are
/// the normalized selection bounds in (line, char-column) space.
fn spans_selection_text(
    lines: &[Vec<(ratatui::style::Style, String)>],
    start_line: usize,
    start_col: usize,
    end_line: usize,
    end_col: usize,
) -> String {
    if start_line >= lines.len() {
        return String::new();
    }
    let mut result = String::new();
    let last = end_line.min(lines.len().saturating_sub(1));
    for (line_idx, spans) in lines
        .iter()
        .enumerate()
        .skip(start_line)
        .take(last - start_line + 1)
    {
        let line_text: String = spans.iter().map(|(_, t)| t.as_str()).collect();
        let chars: Vec<char> = line_text.chars().collect();
        let col_start = if line_idx == start_line { start_col } else { 0 };
        let col_end = if line_idx == end_line {
            end_col.min(chars.len())
        } else {
            chars.len()
        };
        if !result.is_empty() {
            result.push('\n');
        }
        result.extend(&chars[col_start.min(chars.len())..col_end]);
    }
    result
}

impl App {
    /// Maximum valid content_scroll so the last line sits at the bottom edge,
    /// not the top. Falls back to `total - 1` before the first render (height 0).
    pub fn content_scroll_max(&self) -> usize {
        let total = self.display_line_count();
        let vh = (self.content_area.height as usize).max(1);
        total.saturating_sub(vh)
    }

    /// Width of the line-number gutter (fold marker + digits + space), or 0.
    pub fn line_prefix_width(&self) -> usize {
        if self.is_diff {
            return 0;
        }
        if self.is_markdown && !self.show_raw_markdown && !self.markdown_lines.is_empty() {
            return 0;
        }
        if let Some(path) = &self.current_file {
            if self.plugin_content.contains_key(path) {
                return 0;
            }
        }
        let ln = if self.show_line_numbers {
            self.line_count().to_string().len().max(1) + 1
        } else {
            0
        };
        self.fold_gutter_width() + ln
    }

    /// Convert a terminal cell inside `content_area` to a `(buffer_line, buffer_col)` position.
    ///
    /// When word wrap is enabled a single logical line can span multiple visual
    /// rows, so `rel_row` must be translated through the per-line row counts
    /// rather than added directly to `content_scroll`.
    pub fn content_pos(&self, col: u16, row: u16) -> (usize, usize) {
        let ca = self.content_area;
        let rel_row = (row.saturating_sub(ca.y)) as usize;
        let rel_col = (col.saturating_sub(ca.x)) as usize;
        let prefix = self.line_prefix_width();

        if self.word_wrap {
            let raw_wrap = (ca.width as usize).saturating_sub(prefix);
            if let Some(wrap_nz) = NonZeroUsize::new(raw_wrap) {
                let wrap_width = wrap_nz.get();
                let display_total = self.display_line_count();
                let mut visual_remaining = rel_row;
                for display_idx in self.content_scroll..display_total {
                    let physical_idx = self.display_to_physical(display_idx);
                    let display_width: usize = if let Some(ref path) = self.current_file {
                        if let Some(plugin_lines) = self.plugin_content.get(path) {
                            plugin_lines
                                .get(physical_idx)
                                .map(|spans| spans.iter().map(|(_, t)| t.width()).sum())
                                .unwrap_or(0)
                        } else if self.is_markdown && !self.show_raw_markdown {
                            self.markdown_lines
                                .get(physical_idx)
                                .map(|spans| spans.iter().map(|(_, t)| t.width()).sum())
                                .unwrap_or(0)
                        } else {
                            self.line_width(physical_idx).unwrap_or(0)
                        }
                    } else if self.is_markdown && !self.show_raw_markdown {
                        self.markdown_lines
                            .get(physical_idx)
                            .map(|spans| spans.iter().map(|(_, t)| t.width()).sum())
                            .unwrap_or(0)
                    } else {
                        self.line_width(physical_idx).unwrap_or(0)
                    };
                    let visual_rows = display_width.div_ceil(wrap_width).max(1);
                    if visual_remaining < visual_rows {
                        let text_col = rel_col.saturating_sub(prefix);
                        return (physical_idx, visual_remaining * wrap_width + text_col);
                    }
                    visual_remaining -= visual_rows;
                }
                let last_phys = self.display_to_physical(display_total.saturating_sub(1));
                return (last_phys, rel_col.saturating_sub(prefix));
            }
        }

        let display_line = self.content_scroll + rel_row;
        let buf_line = self.display_to_physical(display_line);
        let buf_col = (rel_col + self.content_hscroll).saturating_sub(prefix);
        (buf_line, buf_col)
    }

    /// Extract the currently selected text from the content source.
    pub fn selection_text(&self) -> String {
        let Some(sel) = &self.selection else {
            return String::new();
        };
        if sel.is_empty() {
            return String::new();
        }
        let ((start_line, start_col), (end_line, end_col)) = sel.normalized();
        let total = self.line_count();

        // Plugin-rendered content: extract text from styled spans.
        if let Some(path) = &self.current_file {
            if let Some(plugin_lines) = self.plugin_content.get(path) {
                return spans_selection_text(
                    plugin_lines,
                    start_line,
                    start_col,
                    end_line,
                    end_col,
                );
            }
        }
        if self.is_markdown && !self.show_raw_markdown && !self.markdown_lines.is_empty() {
            return spans_selection_text(
                &self.markdown_lines,
                start_line,
                start_col,
                end_line,
                end_col,
            );
        }
        if self.is_json && self.show_pretty_json && !self.json_pretty_text.is_empty() {
            let lines = &self.json_pretty_text;
            if start_line >= lines.len() {
                return String::new();
            }
            let mut result = String::new();
            let last = end_line.min(lines.len().saturating_sub(1));
            for (line_idx, line) in lines
                .iter()
                .enumerate()
                .skip(start_line)
                .take(last - start_line + 1)
            {
                let chars: Vec<char> = line.chars().collect();
                let col_start = if line_idx == start_line { start_col } else { 0 };
                let col_end = if line_idx == end_line {
                    end_col.min(chars.len())
                } else {
                    chars.len()
                };
                if !result.is_empty() {
                    result.push('\n');
                }
                result.extend(&chars[col_start.min(chars.len())..col_end]);
            }
            return result;
        }
        if self.virtual_file.is_none() {
            // Fallback for inline content (diffs, errors, etc.)
            let lines = &self.content;
            if start_line >= lines.len() {
                return String::new();
            }
            let mut result = String::new();
            let last = end_line.min(lines.len().saturating_sub(1));
            for (line_idx, line) in lines
                .iter()
                .enumerate()
                .skip(start_line)
                .take(last - start_line + 1)
            {
                let chars: Vec<char> = line.chars().collect();
                let col_start = if line_idx == start_line { start_col } else { 0 };
                let col_end = if line_idx == end_line {
                    end_col.min(chars.len())
                } else {
                    chars.len()
                };
                if !result.is_empty() {
                    result.push('\n');
                }
                result.extend(&chars[col_start.min(chars.len())..col_end]);
            }
            return result;
        }
        // VirtualFile path
        if start_line >= total {
            return String::new();
        }
        let mut result = String::new();
        let last = end_line.min(total.saturating_sub(1));
        for line_idx in start_line..=last {
            let Some(line) = self.line_text(line_idx) else {
                continue;
            };
            let chars: Vec<char> = line.chars().collect();
            let col_start = if line_idx == start_line { start_col } else { 0 };
            let col_end = if line_idx == end_line {
                end_col.min(chars.len())
            } else {
                chars.len()
            };
            if !result.is_empty() {
                result.push('\n');
            }
            result.extend(&chars[col_start.min(chars.len())..col_end]);
        }
        result
    }

    /// Clears any active text selection and resets the drag-start position.
    pub(super) fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_start = None;
    }

    /// Sets `content_scroll` to `n`, clamping to `content_scroll_max()`.
    /// Route every raw `content_scroll = …` mutation through this helper.
    pub fn set_content_scroll(&mut self, n: usize) {
        self.content_scroll = n.min(self.content_scroll_max());
    }

    /// Clamps the current `content_scroll` to `content_scroll_max()`.
    /// Call after any content change (reload, fold toggle, markdown toggle, etc.).
    pub fn clamp_content_scroll(&mut self) {
        let max = self.content_scroll_max();
        if self.content_scroll > max {
            self.content_scroll = max;
        }
    }

    /// Number of rows in a page — viewport height minus one overlap row.
    pub fn page_rows(&self) -> usize {
        (self.content_area.height as usize).saturating_sub(1).max(1)
    }

    /// Returns `true` when the content pane uses a text cursor (`active_line`)
    /// instead of raw scroll. Diff, rendered markdown, and plugin-rendered views
    /// are cursorless — they scroll directly  without an active-line cursor.
    pub fn has_text_cursor(&self) -> bool {
        if self.is_diff {
            return false;
        }
        // Rendered markdown has no cursor.
        if self.is_markdown && !self.show_raw_markdown && !self.markdown_lines.is_empty() {
            return false;
        }
        // Plugin-rendered content has no cursor.
        if let Some(path) = &self.current_file {
            if self.plugin_content.contains_key(path) {
                return false;
            }
        }
        true
    }

    /// Unified scroll-into-view helper: nudges `content_scroll` so the given
    /// `display_line` becomes visible. No-op when already visible.
    pub fn scroll_line_into_view(&mut self, display_line: usize) {
        let view_height = (self.content_area.height as usize).max(1);
        if display_line < self.content_scroll {
            self.set_content_scroll(display_line);
        } else if display_line >= self.content_scroll + view_height {
            self.set_content_scroll(display_line.saturating_sub(view_height).saturating_add(1));
        }
    }
}

#[cfg(test)]
#[path = "content_pos_test.rs"]
mod content_pos_test;
