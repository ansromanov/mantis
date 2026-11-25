//! File open/reopen/reveal operations and content (re)loading for `App`.
//!
//! This module is the bridge between tree navigation and the content pane: it
//! opens the selected file, re-reads it after a disk change while preserving
//! scroll position, reveals an arbitrary path in the tree (expanding ancestors
//! as needed), and switches between a file's contents and its working-tree diff
//! in git mode. Heavy work (reading, highlighting, diffing) is delegated to the
//! background `loader` via `compute_file_load`/`compute_diff_load`, with
//! synchronous variants used at startup and for reloads. It also installs the
//! per-file watcher so an open file auto-reloads when it changes on disk.

use std::path::Path;
use std::time::Duration;

use crate::git::GitStatus;

use super::loader::{compute_diff_load, compute_file_load, DiffLoad, FileLoad};
use super::{App, Focus};

impl App {
    /// Suspends automatic follow-mode navigation after user interaction.
    pub(super) fn pause_follow_for_input(&mut self) {
        if self.follow_mode {
            self.follow_pinned = false;
            self.follow_paused_until = Some(self.now() + Duration::from_secs(5));
        }
    }

    /// Re-reads the current file from disk and re-renders it into the content
    /// buffer while preserving scroll position. No-op for historical revision
    /// diffs (which are immutable) and for normal-mode commit diffs, but
    /// refreshes working-tree diffs in git mode.
    pub(super) fn reload_content(&mut self) {
        if self.viewing_revision.is_some() {
            return;
        }
        if self.is_diff && !self.git_mode {
            return;
        }
        if let Some(path) = self.current_file.clone() {
            *self.content_highlight_cache.borrow_mut() = None;
            if self.git_mode {
                self.preserving_scroll(|s| s.show_working_tree_diff(&path));
            } else {
                self.reopen_file(&path);
            }
        }
    }

    /// Runs `f` (which replaces the content buffer) while preserving the
    /// vertical and horizontal scroll position, clamping the vertical scroll to
    /// the new display-line count so it never points past the end of the buffer.
    fn preserving_scroll(&mut self, f: impl FnOnce(&mut Self)) {
        let scroll = self.content_scroll;
        let hscroll = self.content_hscroll;
        f(self);
        self.set_content_scroll(scroll);
        self.content_hscroll = hscroll;
    }

    /// Re-opens `path` via `open_file` while preserving scroll position,
    /// horizontal scroll, and view-mode toggles.
    pub(super) fn reopen_file(&mut self, path: &std::path::Path) {
        let pretty = self.show_pretty_json;
        let csv_table = self.show_csv_table;
        self.preserving_scroll(|s| {
            s.open_file(path);
            if s.is_json {
                s.show_pretty_json = pretty;
            }
            if s.is_csv {
                s.show_csv_table = csv_table;
            }
        });
    }

    /// Displays the working-tree diff of `path` in the content panel
    /// synchronously, using `self.diff_mode` to select the diff variant. When
    /// `compare_base` is set, the diff is computed against that revision instead.
    /// The async navigation path uses `request_working_tree_diff`; both share
    /// `apply_diff_load`.
    pub(super) fn show_working_tree_diff(&mut self, path: &Path) {
        self.invalidate_pending_load();
        let load = compute_diff_load(
            &self.root,
            path,
            &self.theme,
            self.diff_mode,
            self.compare_base.as_deref(),
        );
        self.apply_diff_load(path, load);
    }

    /// Applies a computed working-tree diff to the content panel. Shared by the
    /// synchronous and worker-thread code paths.
    pub(super) fn apply_diff_load(&mut self, path: &Path, load: DiffLoad) {
        // A diff load replaces the complete rendered buffer. Any cached
        // highlighting from the previous source must not survive that swap.
        *self.content_highlight_cache.borrow_mut() = None;
        // Capture whether this is a genuine file switch before we overwrite
        // current_file with the new path.
        let is_new_file = self.current_file.as_deref() != Some(path);
        if is_new_file {
            self.close_diagnostics_picker();
        }

        self.viewing_revision = None;
        self.virtual_file = None;
        self.current_file = Some(path.to_path_buf());
        self.clear_diagnostics_for_path(path);
        self.current_syntax = None;
        self.mark_session_dirty();
        self.rendered_active = false;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_csv = false;
        self.show_csv_table = false;
        self.csv_table_text = Vec::new();
        self.csv_table_lines = Vec::new();
        self.is_diff = true;
        self.clear_fold_state();
        self.file_encoding = None;
        self.file_line_ending = None;
        if is_new_file {
            self.in_file_search = None;
            self.json_query = None;
            self.json_query_original.clear();
            self.set_content_scroll(0);
            self.content_hscroll = 0;
            self.active_line = 0;
            self.show_line_blame = false;
            self.jsonl_expanded.clear();
            self.clear_selection();
            self.rendered_active_path = None;
        } else {
            self.clear_selection();
        }
        self.content_title = Some(load.content_title);
        self.highlighted = load.highlighted;
        self.diff_rows = load.diff_rows;
        self.content_image = None;
        self.content = load.content;
        if self.secret_masked && !self.config.content.mask_secrets {
            self.content = self.secret_original.clone();
            self.secret_masked = false;
        }
        if !is_new_file {
            // Same-file reload: clamp scroll and refresh in-file search.
            self.clamp_content_scroll();
            if self.in_file_search.is_some() {
                self.refresh_in_file_search();
            }
        }
        self.set_file_watch(Some(path));
    }

    /// Shows a "[deleted]" placeholder for a file that was removed from the
    /// working tree but is tracked by git.
    pub(super) fn show_deleted(&mut self, path: &Path) {
        self.invalidate_pending_load();
        self.close_diagnostics_picker();
        self.viewing_revision = None;
        self.in_file_search = None;
        self.clear_diagnostics_for_path(path);
        self.current_file = Some(path.to_path_buf());
        self.current_syntax = None;
        self.mark_session_dirty();
        self.rendered_active = false;
        self.rendered_active_path = None;
        self.is_diff = false;
        self.diff_rows = Vec::new();
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_csv = false;
        self.show_csv_table = false;
        self.csv_table_text = Vec::new();
        self.csv_table_lines = Vec::new();
        self.clear_fold_state();
        self.file_encoding = None;
        self.file_line_ending = None;
        self.virtual_file = None;
        self.content = vec!["[deleted]".into()];
        self.highlighted = Vec::new();
        self.content_title = None;
        self.set_content_scroll(0);
        self.content_hscroll = 0;
        self.active_line = 0;
        self.show_line_blame = false;
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

    /// Reads a file from disk, detects binary, runs syntax highlighting, and
    /// renders JSON/YAML if applicable, synchronously. The async navigation path
    /// uses `request_open_file`; both share `apply_file_load`. Errors and empty
    /// files produce inline messages rather than crashing.
    pub fn open_file(&mut self, path: &Path) {
        self.invalidate_pending_load();
        let load = compute_file_load(
            path,
            &self.highlighter,
            self.config.content.prettify_size_limit,
        );
        self.apply_file_load(path, load);
    }

    pub fn toggle_follow_mode(&mut self) {
        if self.current_file.is_none() {
            self.set_status("follow: no file open");
            return;
        }
        self.follow_mode = !self.follow_mode;
        if self.follow_mode {
            self.follow_pinned = true;
            self.follow_paused_until = None;
            self.follow_candidate = None;
            self.active_line = self.display_line_count().saturating_sub(1);
            self.scroll_active_line_into_view();
            self.set_status("log follow: active");
        } else {
            self.follow_pinned = false;
            self.follow_paused_until = None;
            self.follow_candidate = None;
            self.set_status("log follow: disabled");
        }
    }

    /// Applies a computed file load to the content panel: resets scroll and
    /// selection, then installs the rendered content/highlighting/JSON/YAML
    /// state. Shared by the synchronous and worker-thread code paths.
    pub(super) fn apply_file_load(&mut self, path: &Path, load: FileLoad) {
        let is_log = if load.ok {
            if let Some(ref vf) = load.virtual_file {
                let mut first_lines = Vec::new();
                for i in 0..5 {
                    if let Some(line) = vf.line_text(i) {
                        first_lines.push(line);
                    }
                }
                crate::file::is_log_file(path, &first_lines)
            } else {
                let first_lines: Vec<&str> =
                    load.content.iter().take(5).map(|s| s.as_str()).collect();
                crate::file::is_log_file(path, &first_lines)
            }
        } else {
            false
        };
        self.is_log = is_log;

        self.content_revision += 1;
        self.log_highlight_cache.borrow_mut().clear();
        self.filter_cache.borrow_mut().clear();

        self.is_diff = false;
        self.viewing_revision = None;
        self.diff_rows = Vec::new();
        self.content_title = None;
        // Drop blame popup, active line, scroll, search only when navigating
        // to a different file; a same-file reopen (reload / external edit)
        // preserves all.
        let is_new_file = self.current_file.as_deref() != Some(path);
        let had_filter = self.filter_query.is_some();
        if is_new_file {
            self.close_diagnostics_picker();
            self.file_at_revision = None;
            self.viewing_revision_hash = None;
            // Remember outgoing cursor, restore incoming cursor
            if let Some(old) = self.current_file.clone() {
                self.cursor_positions
                    .insert(old, (self.active_line, self.content_scroll));
            }
            let (line, scroll) = self.cursor_positions.get(path).copied().unwrap_or((0, 0));
            self.in_file_search = None;
            if self.config.content.follow_tail_auto && self.is_log {
                self.follow_mode = true;
                self.follow_pinned = true;
            } else {
                self.follow_mode = false;
                self.follow_pinned = false;
            }
            self.filter_query = None;
            self.filter_bar = None;
            // Assign raw: content not loaded yet, so content_scroll_max() is stale.
            // The `if is_new_file && load.ok` block below clamps once content is in place.
            self.content_scroll = scroll;
            self.content_hscroll = 0;
            self.active_line = line;
            self.show_line_blame = false;
            self.clear_selection();
            self.rendered_active_path = None;
        } else {
            self.clear_selection();
        }

        self.is_json = load.is_json;
        self.is_jsonl = load.is_jsonl;
        self.jsonl_source = load.jsonl_source;
        self.jsonl_display_map = if self.is_jsonl {
            (0..load.content.len()).collect()
        } else {
            Vec::new()
        };
        self.file_encoding = load.encoding;
        self.file_line_ending = load.line_ending;
        self.show_pretty_json = load.show_pretty_json;
        self.json_pretty_text = load.json_pretty_text;
        self.json_pretty_lines = load.json_pretty_lines;
        self.json_path_map = if self.is_json && self.show_pretty_json {
            super::loader::parse_json_value(&load.content.join("\n"))
                .map(|value| crate::json_path::build_path_map(&value))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        self.is_csv = load.is_csv;
        self.show_csv_table = load.show_csv_table;
        self.csv_table_text = load.csv_table_text;
        self.csv_table_lines = load.csv_table_lines;
        self.secret_original = load.secret_original;
        self.secret_masked = load.secret_masked;
        self.secret_revealed = false;
        self.clear_fold_state();
        self.virtual_file = load.virtual_file;
        self.content_image = if self.config.content.image_preview {
            load.image
        } else {
            None
        };
        self.content = load.content;
        if self.secret_masked && !self.config.content.mask_secrets {
            self.content = self.secret_original.clone();
            self.secret_masked = false;
        }
        self.highlighted = if !self.config.content.mask_secrets && !self.secret_original.is_empty()
        {
            self.highlighter.highlight(path, &self.content)
        } else {
            load.highlighted
        };
        if let Some(y) = load.yaml {
            self.fold_regions = y.fold_regions;
            self.yaml_error = y.error;
            self.yaml_anchor_count = y.anchor_count;
            self.yaml_alias_count = y.alias_count;
            self.k8s_summary = y.k8s;
        }
        if !load.fold_regions.is_empty() {
            self.fold_regions = load.fold_regions;
        }
        if load.ok && path.extension().is_some_and(|e| e == "rs") {
            self.symbols.insert(path.to_path_buf(), load.symbols);
        }

        if load.prettify_size_limit_exceeded {
            self.set_status("too large to pretty-print / fold");
        }

        // Same-file reload preserves the active filter, but filter_display_map
        // was built against the old content — rebuild it so filtered lines
        // stay correct after an auto-watch reload or external edit.
        if !is_new_file && load.ok && had_filter {
            self.rebuild_filter_display_map();
        }

        // Clamp restored cursor to current content bounds.
        if load.ok && (is_new_file || (self.follow_mode && self.follow_pinned)) {
            if self.follow_mode && self.follow_pinned {
                self.active_line = self.display_line_count().saturating_sub(1);
                self.content_scroll = self.content_scroll_max();
            } else {
                let max_line = self.display_line_count().saturating_sub(1);
                self.active_line = self.active_line.min(max_line);
                self.clamp_content_scroll();
            }
        }

        if load.ok {
            // Only a newly opened file changes persisted session state; a
            // same-file reload (watcher/refresh) must not dirty the session.
            if is_new_file {
                self.mark_session_dirty();
            }
            self.current_file = Some(path.to_path_buf());
            self.current_syntax = load.syntax_name;
            self.rendered_active = false;
            self.set_file_watch(Some(path));
            self.request_file_diagnostics(path);
            let is_markdown = crate::file::is_markdown_path(path);
            if is_markdown && !self.show_raw_markdown {
                self.install_native_markdown(path);
            }
            if is_new_file {
                self.push_recent(path.to_path_buf());
            } else {
                // Same-file reload: clamp scroll and refresh in-file search.
                self.clamp_content_scroll();
                if self.in_file_search.is_some() {
                    self.refresh_in_file_search();
                }
            }
        } else {
            // Don't keep current_file pointing at a different file that is
            // still displayed, but preserve it when the same file fails to
            // reload so that the next successful reload sees is_new_file=false
            // and correctly preserves blame/visual-line state.
            if self.current_file.as_deref() != Some(path) {
                self.current_file = None;
            }
            self.current_syntax = None;
            self.mark_session_dirty();
            self.set_file_watch(None);
        }
    }
}

#[cfg(test)]
#[path = "file_ops_test.rs"]
mod tests;
