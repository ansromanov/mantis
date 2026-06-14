use std::path::Path;

use notify::{EventKind, RecursiveMode, Watcher};

use crate::file::is_binary_bytes;
use crate::git::GitStatus;
use crate::markdown;
use crate::search::HistoryState;
use crate::virtual_file::VirtualFile;

use super::{diff_line_style, App, Focus};

impl App {
    /// Re-reads the current file from disk and re-renders it into the content
    /// buffer while preserving scroll position. No-op for commit diffs (which
    /// are immutable), but refreshes working-tree diffs in git mode.
    pub(super) fn reload_content(&mut self) {
        if self.is_diff && !self.git_mode {
            return;
        }
        if let Some(path) = self.current_file.clone() {
            if self.git_mode {
                self.preserving_scroll(|s| s.show_working_tree_diff(&path));
            } else {
                self.reopen_file(&path);
            }
        }
    }

    /// Runs `f` (which replaces the content buffer) while preserving the
    /// vertical and horizontal scroll position, clamping the vertical scroll to
    /// the new line count so it never points past the end of the buffer.
    fn preserving_scroll(&mut self, f: impl FnOnce(&mut Self)) {
        let scroll = self.content_scroll;
        let hscroll = self.content_hscroll;
        f(self);
        self.content_scroll = scroll.min(self.line_count().saturating_sub(1));
        self.content_hscroll = hscroll;
    }

    /// Re-opens `path` via `open_file` while preserving scroll position,
    /// horizontal scroll, and view-mode toggles.
    pub(super) fn reopen_file(&mut self, path: &std::path::Path) {
        let raw = self.show_raw_markdown;
        let pretty = self.show_pretty_json;
        self.preserving_scroll(|s| {
            s.open_file(path);
            s.show_raw_markdown = raw;
            if s.is_json {
                s.show_pretty_json = pretty;
            }
        });
    }

    /// Sets up a filesystem watcher on the parent directory of `path` so that
    /// `drain_file_watch` can detect external edits. Clears any previous watch.
    /// Watches the parent directory (not the file) to catch atomic-save renames.
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

    /// Drains all pending file-watch events and returns `true` if the watched
    /// file was modified, created, or deleted since the last check.
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

    /// Displays the working-tree diff of `path` (relative to HEAD) in the
    /// content panel, using `diff_line_style` for per-line coloring.
    pub(super) fn show_working_tree_diff(&mut self, path: &Path) {
        self.in_file_search = None;
        self.virtual_file = None;
        let lines = crate::git::working_tree_diff(&self.root, path);
        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        self.current_file = Some(path.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_diff = true;
        self.clear_yaml_state();
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        self.content_title = Some(format!(" working diff — {} ", rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.diff_rows = crate::diff::parse_side_by_side(&lines);
        self.content = lines;
        self.set_file_watch(Some(path));
    }

    /// Shows a "[deleted]" placeholder for a file that was removed from the
    /// working tree but is tracked by git.
    pub(super) fn show_deleted(&mut self, path: &Path) {
        self.in_file_search = None;
        self.current_file = Some(path.to_path_buf());
        self.is_diff = false;
        self.diff_rows = Vec::new();
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.clear_yaml_state();
        self.virtual_file = None;
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

    /// Reads a file from disk, detects binary/markdown, runs syntax
    /// highlighting, and renders markdown if applicable. Errors and empty files
    /// produce inline messages rather than crashing.
    pub fn open_file(&mut self, path: &Path) {
        self.in_file_search = None;
        self.is_diff = false;
        self.diff_rows = Vec::new();
        self.content_title = None;
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        self.is_markdown = matches!(ext, "md" | "markdown");
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_json = ext == "json";
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        let is_yaml = matches!(ext, "yaml" | "yml");
        self.clear_yaml_state();

        // Try memory-mapped virtual file first (lazy, no full content in memory).
        // Markdown, JSON, and YAML are excluded: they need full content for rendering/validation.
        if !self.is_markdown && !self.is_json && !is_yaml {
            if let Some(vf) = VirtualFile::open(path) {
                self.current_file = Some(path.to_path_buf());
                self.set_file_watch(Some(path));
                self.virtual_file = Some(vf);
                self.content = Vec::new();
                self.highlighted = Vec::new();
                return;
            }
        }

        // Fallback: read the file into memory (small files, binary check, etc.)
        self.virtual_file = None;
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.current_file = None;
                self.set_file_watch(None);
                self.content = vec![format!("[error: {}]", e)];
                self.highlighted = Vec::new();
                return;
            }
        };
        self.current_file = Some(path.to_path_buf());
        self.set_file_watch(Some(path));
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
            if is_yaml {
                self.yaml_fold_regions = crate::yaml_fold::detect_fold_regions(&self.content);
                self.validate_yaml(&s);
                let lines = self.content.clone();
                self.count_yaml_anchors_aliases(&lines);
            }
            self.highlighted = self.highlighter.highlight(path, &self.content);
            if self.is_markdown {
                self.markdown_lines = markdown::render(&s, &self.theme);
            }
            if self.is_json {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&s) {
                    if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                        let pretty_lines: Vec<String> =
                            pretty.lines().map(|l| l.to_owned()).collect();
                        self.json_pretty_lines = self.highlighter.highlight(path, &pretty_lines);
                        self.json_pretty_text = pretty_lines;
                        self.show_pretty_json = true;
                    }
                }
            }
        }
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

    /// Loads a diff (from git history) into the content panel with styled
    /// per-line markers. Sets `is_diff = true` so the line-number gutter is
    /// hidden and the diff stays read-only.
    fn show_diff(&mut self, file: &Path, short: &str, lines: Vec<String>) {
        self.in_file_search = None;
        self.virtual_file = None;
        self.current_file = Some(file.to_path_buf());
        self.is_markdown = false;
        self.show_raw_markdown = false;
        self.markdown_lines = Vec::new();
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_diff = true;
        self.clear_yaml_state();
        self.content_scroll = 0;
        self.content_hscroll = 0;
        self.clear_selection();
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        self.content_title = Some(format!(" diff {} — {} ", short, rel.display()));
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.diff_rows = crate::diff::parse_side_by_side(&lines);
        self.content = lines;
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }
}
