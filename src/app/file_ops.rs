use std::path::Path;

use notify::{EventKind, RecursiveMode, Watcher};

use crate::file::is_binary_bytes;
use crate::git::GitStatus;
use crate::markdown;
use crate::search::HistoryState;

use super::{diff_line_style, App, Focus};

impl App {
    pub(super) fn reload_content(&mut self) {
        // Commit diffs are transient; don't clobber them on refresh.
        // Working-tree diffs in git mode should be refreshed (working tree changes).
        if self.is_diff && !self.git_mode {
            return;
        }
        if let Some(path) = self.current_file.clone() {
            let scroll = self.content_scroll;
            let hscroll = self.content_hscroll;
            if self.git_mode {
                self.show_working_tree_diff(&path);
            } else {
                let raw = self.show_raw_markdown;
                self.open_file(&path);
                self.show_raw_markdown = raw;
            }
            self.content_scroll = scroll.min(self.content_line_count().saturating_sub(1));
            self.content_hscroll = hscroll;
        }
    }

    fn set_file_watch(&mut self, path: Option<&Path>) {
        self.file_watcher = None;
        self.file_watch_rx = None;
        self.file_watch_path = None;
        let Some(p) = path else { return };
        // Watch the parent directory rather than the file itself so that
        // atomic-save editors (those that write a temp file and rename it over
        // the original) still trigger events after the inode is replaced.
        let Some(dir) = p.parent() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            self.file_watcher = Some(watcher);
            self.file_watch_rx = Some(rx);
            self.file_watch_path = Some(p.to_path_buf());
        }
    }

    pub(super) fn drain_file_watch(&self) -> bool {
        let (Some(rx), Some(watched)) = (&self.file_watch_rx, &self.file_watch_path) else {
            return false;
        };
        let mut changed = false;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                let affects_watched = evt.paths.iter().any(|p| p == watched);
                if affects_watched
                    && matches!(
                        evt.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    )
                {
                    changed = true;
                }
            }
        }
        changed
    }

    pub(super) fn show_working_tree_diff(&mut self, path: &Path) {
        let lines = crate::git::working_tree_diff(&self.root, path);
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        self.current_file = Some(path.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_diff = true;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        self.content_title = Some(format!(" working diff — {} ", rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.content = lines;
        self.set_file_watch(Some(path));
    }

    pub(super) fn show_deleted(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
        self.is_diff = false;
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.content = vec!["[deleted]".into()];
        self.highlighted = Vec::new();
        self.markdown_lines = Vec::new();
        self.content_title = None;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        self.set_file_watch(None);
    }

    /// Opens the currently selected file and selects it in the tree, expanding
    /// parent directories as needed. Used when a file path is passed on the
    /// command line.
    pub fn open_and_reveal(&mut self, path: &Path) {
        if !path.exists() && self.git_status_map.get(path) == Some(&GitStatus::Deleted) {
            self.show_deleted(path);
        } else {
            self.open_file(path);
        }
        self.reveal_in_tree(path);
        self.focus = Focus::Content;
    }

    pub fn open_file(&mut self, path: &Path) {
        self.current_file = Some(path.to_path_buf());
        self.is_diff = false;
        self.content_title = None;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.is_markdown = matches!(ext, "md" | "markdown");
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();

        // Read the file once: classify it as binary and decode it from the
        // same bytes, rather than reading the whole file twice.
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.content = vec![format!("[error: {}]", e)];
                self.highlighted = Vec::new();
                return;
            }
        };
        if is_binary_bytes(&bytes) {
            self.content = vec!["[binary file]".into()];
            self.highlighted = Vec::new();
            return;
        }
        let s = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(_) => {
                self.content = vec!["[binary file]".into()];
                self.highlighted = Vec::new();
                return;
            }
        };
        self.content = s.lines().map(|l| l.to_owned()).collect();
        if self.content.is_empty() {
            self.content = vec!["[empty file]".into()];
            self.highlighted = Vec::new();
        } else {
            self.highlighted = self.highlighter.highlight(path, &self.content);
            if self.is_markdown {
                self.markdown_lines = markdown::render(&s, &self.theme);
            }
        }
        self.set_file_watch(Some(path));
    }

    /// Opens the git history of the currently displayed file as a picker.
    /// Does nothing if no file is open or the file has no tracked history.
    pub(super) fn open_file_history(&mut self) {
        let Some(file) = self.current_file.clone() else {
            return;
        };
        let commits = crate::git::file_log(&self.root, &file);
        if commits.is_empty() {
            return;
        }
        self.history = Some(HistoryState::new(file, commits));
    }

    /// Loads the diff of the selected revision into the content panel.
    pub(super) fn show_selected_revision(&mut self) {
        let picked = self.history.as_ref().and_then(|h| {
            h.selected_commit()
                .map(|c| (c.hash.clone(), c.short.clone(), h.file.clone()))
        });
        self.history = None;
        if let Some((hash, short, file)) = picked {
            let diff = crate::git::file_diff(&self.root, &hash, &file);
            self.show_diff(&file, &short, diff);
        }
    }

    fn show_diff(&mut self, file: &Path, short: &str, lines: Vec<String>) {
        self.current_file = Some(file.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_diff = true;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        self.content_title = Some(format!(" diff {} — {} ", short, rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.content = lines;
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }

    pub(super) fn content_line_count(&self) -> usize {
        if self.is_markdown && !self.show_raw_markdown {
            self.markdown_lines.len()
        } else {
            self.content.len()
        }
    }

    /// Width of the line-number gutter (digits + space), or 0 when there is none.
    pub fn line_prefix_width(&self) -> usize {
        if self.is_diff || (self.is_markdown && !self.show_raw_markdown) {
            0
        } else {
            self.content.len().to_string().len().max(1) + 1
        }
    }

    /// Convert a terminal cell inside `content_area` to a `(buffer_line, buffer_col)` position.
    pub fn content_pos(&self, col: u16, row: u16) -> (usize, usize) {
        let ca = self.content_area;
        let rel_row = (row.saturating_sub(ca.y)) as usize;
        let rel_col = (col.saturating_sub(ca.x)) as usize;
        let buf_line = self.content_scroll + rel_row;
        let prefix = self.line_prefix_width();
        let buf_col = (rel_col + self.content_hscroll).saturating_sub(prefix);
        (buf_line, buf_col)
    }

    /// Extract the currently selected text from `self.content`.
    pub fn selection_text(&self) -> String {
        let Some(sel) = &self.selection else {
            return String::new();
        };
        if sel.is_empty() {
            return String::new();
        }
        let ((start_line, start_col), (end_line, end_col)) = sel.normalized();

        if self.is_markdown && !self.show_raw_markdown {
            let lines = &self.markdown_lines;
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
            return result;
        }

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
        result
    }

    pub(super) fn clear_selection(&mut self) {
        self.selection = None;
        self.drag_start = None;
    }

    pub(super) fn reveal_in_tree(&mut self, path: &Path) {
        let mut current = path.parent();
        while let Some(dir) = current {
            if dir == self.root {
                break;
            }
            if dir.starts_with(&self.root) {
                self.expanded.insert(dir.to_path_buf());
            } else {
                break;
            }
            current = dir.parent();
        }
        self.rebuild();
        if let Some(i) = self.nodes.iter().position(|n| n.path == path) {
            self.tree_selected = i;
        }
    }
}
