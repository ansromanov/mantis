//! Recent files, bookmarks, file search, history, revision views and blame-commit navigation.

use std::path::{Path, PathBuf};

use crate::search::{HistoryState, RecentFilesState};

use super::{diff_line_style, App, Focus};

impl App {
    /// Adds `path` to the front of the recent-files ring, deduplicating
    /// and capping to `config.recent_files_count`.
    pub(super) fn push_recent(&mut self, path: PathBuf) {
        self.recent_ring.retain(|p| p != &path);
        self.recent_ring.insert(0, path);
        let cap = self.config.recent_files_count.max(1);
        self.recent_ring.truncate(cap);
    }

    /// Opens the recent-files overlay. Does nothing when the ring is empty
    /// or every entry is the currently open file.
    pub(super) fn open_recent_files(&mut self) {
        let current = self.current_file.clone();
        let paths: Vec<PathBuf> = self
            .recent_ring
            .iter()
            .filter(|p| Some(*p) != current.as_ref())
            .cloned()
            .collect();
        if paths.is_empty() {
            return;
        }
        self.recent_files = Some(RecentFilesState::new(paths));
    }

    /// Opens the file selected in the recent-files overlay and closes it.
    pub(super) fn activate_recent_selection(&mut self) {
        let path = self
            .recent_files
            .as_ref()
            .and_then(|r| r.selected_path().cloned());
        self.recent_files = None;
        if let Some(path) = path {
            self.open_and_reveal(&path);
        }
    }

    /// Toggles a bookmark for the currently open file and persists the change.
    pub(super) fn toggle_bookmark(&mut self) {
        let Some(path) = self.current_file.clone() else {
            self.set_status("bookmark: no file is open");
            return;
        };
        self.toggle_bookmark_path(path);
    }

    /// Toggles a bookmark for `path` and persists the change.
    pub(super) fn toggle_bookmark_path(&mut self, path: PathBuf) {
        if let Some(index) = self.bookmark_paths.iter().position(|p| p == &path) {
            self.bookmark_paths.remove(index);
            self.set_status("bookmark removed");
        } else {
            self.bookmark_paths.push(path);
            self.set_status("bookmark added");
        }
        self.mark_session_dirty();
    }

    /// Opens the bookmark picker when at least one bookmark exists.
    pub(super) fn open_bookmarks(&mut self) {
        if self.bookmark_paths.is_empty() {
            self.set_status("bookmarks: no files bookmarked");
            return;
        }
        self.bookmarks = Some(RecentFilesState::new(self.bookmark_paths.clone()));
    }

    /// Opens the selected bookmark and closes the picker.
    pub(super) fn activate_bookmark_selection(&mut self) {
        let path = self
            .bookmarks
            .as_ref()
            .and_then(|picker| picker.selected_path().cloned());
        self.bookmarks = None;
        if let Some(path) = path {
            self.open_and_reveal(&path);
        }
    }

    /// Opens the global fuzzy file-name picker (SearchState in Files mode),
    /// scoped to git-changed files when in git mode. Focus-independent.
    pub fn open_file_search(&mut self) {
        let search = self.new_search_state(false);
        self.open_palette_search(search);
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
            let diff = crate::git::file_commit_diff(&self.root, &hash, &file);
            self.show_diff(&file, &short, &diff, Some(&hash));
        }
    }

    /// Loads a diff (from git history) into the content panel with styled
    /// per-line markers. Sets `is_diff = true` so the line-number gutter is
    /// hidden and the diff stays read-only.
    pub(super) fn show_diff(
        &mut self,
        file: &Path,
        short: &str,
        lines: &[String],
        full_hash: Option<&str>,
    ) {
        self.invalidate_pending_load();
        self.in_file_search = None;
        self.virtual_file = None;
        self.current_file = Some(file.to_path_buf());
        self.mark_session_dirty();
        self.rendered_active = false;
        self.rendered_active_path = None;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_csv = false;
        self.show_csv_table = false;
        self.csv_table_text = Vec::new();
        self.csv_table_lines = Vec::new();
        self.is_diff = true;
        self.viewing_revision = Some(short.to_string());
        self.viewing_revision_hash = full_hash.map(|h| h.to_string());
        self.file_at_revision = None;
        self.clear_fold_state();
        self.file_encoding = None;
        self.file_line_ending = None;
        self.set_content_scroll(0);
        self.content_hscroll = 0;
        self.active_line = 0;
        self.show_line_blame = false;
        self.clear_selection();
        let rel = file.strip_prefix(&self.root).unwrap_or(file);
        self.content_title = Some(format!(" diff {} — {} ", short, rel.display()));
        let lines: Vec<String> = lines
            .iter()
            .map(|line| crate::ansi::sanitize_terminal_text(line))
            .collect();
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.diff_rows = crate::diff::parse_side_by_side(&lines);
        self.content = lines;
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }

    /// Opens the repository-wide commit log as a picker overlay.
    /// Does nothing if not in a git repo.
    pub(super) fn open_repo_log(&mut self) {
        if self.git_info.is_none() {
            return;
        }
        self.repo_log = Some(crate::search::RepoLogState::new(self.root.clone()));
    }

    /// Enters compare mode for the selected commit in the repo log overlay.
    /// Filters the tree to files changed since that commit.
    pub(super) fn show_repo_log_compare(&mut self) {
        let picked = self
            .repo_log
            .as_ref()
            .and_then(|r| r.selected_commit().map(|c| c.hash.clone()));
        self.repo_log = None;
        if let Some(hash) = picked {
            self.enter_commit_mode(hash);
        }
    }

    /// Toggles between the revision diff view and the file content at that
    /// revision. When in a revision diff (`viewing_revision` is set), loads the
    /// file via `git show <rev>:<path>` and stores the diff state for instant
    /// toggle-back. When viewing a file-at-revision snapshot, restores the
    /// saved diff without re-fetching.
    pub(super) fn toggle_file_revision(&mut self) {
        if self.file_at_revision.is_some() {
            // Currently showing file-at-revision snapshot → restore saved diff.
            if let Some(snapshot) = self.file_at_revision.take() {
                if let Some(saved) = snapshot.saved_diff {
                    self.invalidate_pending_load();
                    self.in_file_search = None;
                    self.virtual_file = None;
                    self.is_diff = true;
                    self.viewing_revision = Some(snapshot.short);
                    self.viewing_revision_hash = Some(snapshot.hash);
                    self.clear_fold_state();
                    self.file_encoding = None;
                    self.file_line_ending = None;
                    self.is_json = false;
                    self.show_pretty_json = false;
                    self.json_pretty_text = Vec::new();
                    self.json_pretty_lines = Vec::new();
                    self.is_csv = false;
                    self.show_csv_table = false;
                    self.csv_table_text = Vec::new();
                    self.csv_table_lines = Vec::new();
                    self.content_scroll = saved.content_scroll;
                    self.active_line = saved.active_line;
                    self.diff_side_by_side = saved.side_by_side;
                    self.show_line_blame = false;
                    self.show_blame = self.blame_before_commit;
                    self.blame_before_commit = false;
                    self.clear_selection();
                    self.content_title = Some(saved.content_title);
                    self.highlighted = saved.highlighted;
                    self.diff_rows = saved.diff_rows;
                    self.content = saved.content;
                    self.set_file_watch(None);
                }
            }
            return;
        }

        // Currently in a revision diff → load file at that revision.
        let Some(rev) = self.viewing_revision.clone() else {
            self.set_status("toggle file at revision: not viewing a revision diff");
            return;
        };
        let Some(ref hash) = self.viewing_revision_hash.clone() else {
            self.set_status("toggle file at revision: commit hash unknown");
            return;
        };
        let Some(ref path) = self.current_file.clone() else {
            return;
        };
        let Some(bytes) = crate::git::file_at_rev(&self.root, hash, path) else {
            self.set_status(format!(
                "file not found at revision {}",
                rev.chars().take(7).collect::<String>()
            ));
            return;
        };
        if crate::file::is_binary_bytes(&bytes) {
            self.set_status("file at revision is binary");
            return;
        }
        let text = String::from_utf8_lossy(&bytes);
        let text = if text.contains('\r') {
            text.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            text.into()
        };
        let lines: Vec<String> = text.lines().map(|l| l.to_owned()).collect();
        if lines.is_empty() {
            self.set_status("file at revision is empty");
            return;
        }

        // Save the current diff state for toggle-back.
        let saved_diff = super::types::SavedDiffState {
            content: self.content.clone(),
            highlighted: self.highlighted.clone(),
            diff_rows: self.diff_rows.clone(),
            content_title: self.content_title.clone().unwrap_or_default(),
            content_scroll: self.content_scroll,
            active_line: self.active_line,
            side_by_side: self.diff_side_by_side,
        };

        // Load the file-at-revision snapshot.
        self.invalidate_pending_load();
        self.in_file_search = None;
        self.virtual_file = None;
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
        self.set_content_scroll(0);
        self.content_hscroll = 0;
        self.active_line = 0;
        self.show_line_blame = false;
        self.clear_selection();

        let rel = path.strip_prefix(&self.root).unwrap_or(path);
        let short_display: String = rev.chars().take(7).collect();
        self.content_title = Some(format!(" {} @ {} ", rel.display(), short_display));
        self.highlighted = self.highlighter.highlight(path, &lines);
        self.current_syntax = self.highlighter.syntax_name(path);
        self.content = lines;
        self.file_at_revision = Some(super::types::FileAtRevision {
            short: rev,
            hash: hash.clone(),
            saved_diff: Some(saved_diff),
        });
        self.set_file_watch(None);
    }

    /// Opens the file at the commit recorded on the active blame line: loads
    /// that commit's diff, then immediately toggles to the file-at-revision
    /// snapshot. Emits a status message when the file or blame data is missing.
    pub(super) fn open_blame_commit_at_active_line(&mut self) {
        let physical = self.display_to_physical(self.active_line);
        let Some(path) = self.current_file.clone() else {
            self.set_status("blame open commit: no file open");
            return;
        };
        let blame_lines = crate::git::file_blame(&self.root, &path);
        let lineno = physical as u32 + 1;
        let Some(bl) = blame_lines.iter().find(|b| b.line_no == lineno) else {
            self.set_status("blame open commit: no blame data for active line");
            return;
        };
        let hash = bl.commit_hash.clone();
        let short = bl.short_hash.clone();
        self.blame_before_commit = self.show_blame;
        self.show_blame = false;
        let diff = crate::git::file_diff(&self.root, &hash, &path);
        self.show_diff(&path, &short, &diff, Some(&hash));
        self.toggle_file_revision();
    }
}
