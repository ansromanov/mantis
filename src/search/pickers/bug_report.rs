//! Picker: bug report.

/// Which field of the bug report modal currently receives keyboard input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BugReportFocus {
    Title,
    Description,
}

/// State for the bug report modal.
/// Holds the typed text lines, the cursor position, and the viewport scroll offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BugReportState {
    pub title: String,
    pub title_cursor: usize,
    pub focus: BugReportFocus,
    pub text: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub scroll_top: usize,
    pub preview_scroll: crate::scroll::ScrollState,
    pub diagnostics_markdown: String,
}

impl Default for BugReportState {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl BugReportState {
    pub fn new(diagnostics_markdown: String) -> Self {
        let title = "App Bug Report".to_string();
        let title_cursor = title.chars().count();
        BugReportState {
            title,
            title_cursor,
            focus: BugReportFocus::Description,
            text: vec![String::new()],
            cursor_row: 0,
            cursor_col: 0,
            scroll_top: 0,
            preview_scroll: crate::scroll::ScrollState::new(),
            diagnostics_markdown,
        }
    }

    /// The title to use when submitting or previewing the report: the trimmed
    /// user-entered title, or "App Bug Report" when it is missing/blank.
    pub fn resolved_title(&self) -> &str {
        let trimmed = self.title.trim();
        if trimmed.is_empty() {
            "App Bug Report"
        } else {
            trimmed
        }
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            BugReportFocus::Title => BugReportFocus::Description,
            BugReportFocus::Description => BugReportFocus::Title,
        };
    }

    pub fn title_insert_char(&mut self, c: char) {
        let char_len = self.title.chars().count();
        if self.title_cursor > char_len {
            self.title_cursor = char_len;
        }
        let byte_idx = self
            .title
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.title_cursor)
            .unwrap_or(self.title.len());
        self.title.insert(byte_idx, c);
        self.title_cursor += 1;
    }

    pub fn title_backspace(&mut self) {
        if self.title_cursor == 0 {
            return;
        }
        let char_idx = self.title_cursor - 1;
        let byte_idx = self
            .title
            .char_indices()
            .map(|(i, _)| i)
            .nth(char_idx)
            .unwrap_or(self.title.len());
        self.title.remove(byte_idx);
        self.title_cursor -= 1;
    }

    pub fn title_delete(&mut self) {
        let char_len = self.title.chars().count();
        if self.title_cursor >= char_len {
            return;
        }
        let byte_idx = self
            .title
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.title_cursor)
            .unwrap_or(self.title.len());
        self.title.remove(byte_idx);
    }

    pub fn title_move_left(&mut self) {
        if self.title_cursor > 0 {
            self.title_cursor -= 1;
        }
    }

    pub fn title_move_right(&mut self) {
        let char_len = self.title.chars().count();
        if self.title_cursor < char_len {
            self.title_cursor += 1;
        }
    }

    pub fn title_move_home(&mut self) {
        self.title_cursor = 0;
    }

    pub fn title_move_end(&mut self) {
        self.title_cursor = self.title.chars().count();
    }

    pub fn insert_char(&mut self, c: char) {
        if self.text.is_empty() {
            self.text.push(String::new());
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col > char_len {
            self.cursor_col = char_len;
        }
        let byte_idx = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        line.insert(byte_idx, c);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        if self.text.is_empty() {
            self.text.push(String::new());
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col > char_len {
            self.cursor_col = char_len;
        }
        let byte_idx = line
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.cursor_col)
            .unwrap_or(line.len());
        let next_line = line.split_off(byte_idx);
        self.text.insert(self.cursor_row + 1, next_line);
        self.cursor_row += 1;
        self.cursor_col = 0;
    }

    pub fn backspace(&mut self) {
        if self.text.is_empty() {
            return;
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        if self.cursor_col > 0 {
            let line = &mut self.text[self.cursor_row];
            let char_len = line.chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
            let char_idx = self.cursor_col - 1;
            let byte_idx = line
                .char_indices()
                .map(|(i, _)| i)
                .nth(char_idx)
                .unwrap_or(line.len());
            line.remove(byte_idx);
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let current_line = self.text.remove(self.cursor_row);
            self.cursor_row -= 1;
            let prev_line = &mut self.text[self.cursor_row];
            let prev_len = prev_line.chars().count();
            prev_line.push_str(&current_line);
            self.cursor_col = prev_len;
        }
    }

    pub fn delete(&mut self) {
        if self.text.is_empty() {
            return;
        }
        if self.cursor_row >= self.text.len() {
            self.cursor_row = self.text.len() - 1;
        }
        let line = &mut self.text[self.cursor_row];
        let char_len = line.chars().count();
        if self.cursor_col < char_len {
            let byte_idx = line
                .char_indices()
                .map(|(i, _)| i)
                .nth(self.cursor_col)
                .unwrap_or(line.len());
            line.remove(byte_idx);
        } else if self.cursor_row + 1 < self.text.len() {
            let next_line = self.text.remove(self.cursor_row + 1);
            let line = &mut self.text[self.cursor_row];
            line.push_str(&next_line);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.text[self.cursor_row].chars().count();
        }
    }

    pub fn move_right(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let char_len = self.text[self.cursor_row].chars().count();
        if self.cursor_col < char_len {
            self.cursor_col += 1;
        } else if self.cursor_row + 1 < self.text.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let char_len = self.text[self.cursor_row].chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row + 1 < self.text.len() {
            self.cursor_row += 1;
            let char_len = self.text[self.cursor_row].chars().count();
            if self.cursor_col > char_len {
                self.cursor_col = char_len;
            }
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        if !self.text.is_empty() {
            self.cursor_col = self.text[self.cursor_row].chars().count();
        }
    }

    fn visual_row_count(line: &str, width: usize) -> usize {
        if width == 0 {
            return 1;
        }
        let n = line.chars().count();
        if n == 0 {
            1
        } else {
            n.div_ceil(width)
        }
    }

    /// Visual (rendered) row index of the cursor for the given character-cell edit width.
    pub fn cursor_visual_row(&self, width: usize) -> usize {
        let mut visual = 0;
        for (i, line) in self.text.iter().enumerate() {
            if i == self.cursor_row {
                return visual + self.cursor_col / width.max(1);
            }
            visual += Self::visual_row_count(line, width);
        }
        visual
    }

    /// Total visual (rendered) rows across all logical lines for the given edit width.
    pub fn total_visual_rows(&self, width: usize) -> usize {
        let mut total = 0;
        for (i, line) in self.text.iter().enumerate() {
            let n = line.chars().count();
            let mut rows = Self::visual_row_count(line, width);
            if i == self.cursor_row && self.cursor_col == n && n > 0 && n % width.max(1) == 0 {
                rows += 1;
            }
            total += rows;
        }
        total
    }

    pub fn clamp_scroll(&mut self, height: usize, width: usize) {
        if height == 0 || width == 0 {
            return;
        }
        let cursor_vis = self.cursor_visual_row(width);
        if cursor_vis < self.scroll_top {
            self.scroll_top = cursor_vis;
        } else if cursor_vis >= self.scroll_top + height {
            self.scroll_top = cursor_vis - height + 1;
        }
        let total_vis = self.total_visual_rows(width);
        let max_scroll = total_vis.saturating_sub(height);
        if self.scroll_top > max_scroll {
            self.scroll_top = max_scroll;
        }
    }
}
