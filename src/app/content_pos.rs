use std::num::NonZeroUsize;

use unicode_width::UnicodeWidthStr;

use super::App;

impl App {
    /// Maximum valid content_scroll so the last line sits at the bottom edge,
    /// not the top. Falls back to `total - 1` before the first render (height 0).
    pub fn content_scroll_max(&self) -> usize {
        let total = self.line_count();
        let vh = (self.content_area.height as usize).max(1);
        total.saturating_sub(vh)
    }

    /// Width of the line-number gutter (digits + space), or 0 when there is none.
    pub fn line_prefix_width(&self) -> usize {
        if self.is_diff || (self.is_markdown && !self.show_raw_markdown) {
            0
        } else {
            self.line_count().to_string().len().max(1) + 1
        }
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
                let is_md = self.is_markdown && !self.show_raw_markdown;
                let total = self.line_count();
                let mut visual_remaining = rel_row;
                for logical_idx in self.content_scroll..total {
                    let display_width: usize = if is_md {
                        self.markdown_lines
                            .get(logical_idx)
                            .map(|spans| spans.iter().map(|(_, t)| t.width()).sum())
                            .unwrap_or(0)
                    } else {
                        self.line_width(logical_idx).unwrap_or(0)
                    };
                    let visual_rows = display_width.div_ceil(wrap_width).max(1);
                    if visual_remaining < visual_rows {
                        let text_col = rel_col.saturating_sub(prefix);
                        return (logical_idx, visual_remaining * wrap_width + text_col);
                    }
                    visual_remaining -= visual_rows;
                }
                return (total.saturating_sub(1), rel_col.saturating_sub(prefix));
            }
        }

        let buf_line = self.content_scroll + rel_row;
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

        if self.is_markdown && !self.show_raw_markdown {
            if start_line >= self.markdown_lines.len() {
                return String::new();
            }
            let mut result = String::new();
            let last = end_line.min(self.markdown_lines.len().saturating_sub(1));
            for (line_idx, spans) in self
                .markdown_lines
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
}
