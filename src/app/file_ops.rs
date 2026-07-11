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

use std::path::{Path, PathBuf};

use notify::{EventKind, RecursiveMode, Watcher};

use crate::git::GitStatus;
use crate::search::{HistoryState, RecentFilesState, SearchState};

use super::loader::{compute_diff_load, compute_file_load, DiffLoad, FileLoad};
use super::{diff_line_style, App, Focus};

impl App {
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
        self.preserving_scroll(|s| {
            s.open_file(path);
            if s.is_json {
                s.show_pretty_json = pretty;
            }
        });
    }

    /// Sets up a filesystem watcher on the parent directory of `path` so that
    /// `drain_file_watch` can detect external edits. Clears any previous watch.
    /// Watches the parent directory (not the file) to catch atomic-save renames.
    pub(super) fn set_file_watch(&mut self, path: Option<&Path>) {
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

    /// Installs a recursive filesystem watcher on the view root so that
    /// `drain_root_watch` can detect tree changes (files added/removed, git
    /// status changes from edits anywhere in the repo) and drive a debounced
    /// reload instead of a blind periodic one. Best-effort: if the watcher
    /// cannot be installed (e.g. the OS hits a watch-descriptor limit on a very
    /// large tree) the field stays `None` and `tick` falls back to the timer.
    pub fn watch_root(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(&self.root, RecursiveMode::Recursive).is_ok() {
            self.root_watcher = Some(watcher);
            self.root_watch_rx = Some(rx);
        }
    }

    /// Installs a non-recursive watcher on the parent directory of the config
    /// file so edits to `mantis.toml` are detected and hot-reloaded. Best-effort:
    /// if the watcher cannot be installed (e.g. the directory does not exist) the
    /// fields stay `None` and no live-reload happens — the user simply does not
    /// get the feature.
    pub fn install_config_watcher(&mut self) {
        let Some(ref path) = self.config_path else {
            return;
        };
        let Some(dir) = path.parent() else {
            return;
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let Ok(mut watcher) = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) else {
            return;
        };
        if watcher.watch(dir, RecursiveMode::NonRecursive).is_ok() {
            self.config_watcher = Some(watcher);
            self.config_watch_rx = Some(rx);
        }
    }

    /// Drains all pending root-watch events and returns `true` if any of them
    /// created, modified, or removed a path since the last check. Access-only
    /// events are ignored so merely reading files doesn't trigger reloads.
    pub(super) fn drain_root_watch(&self) -> bool {
        let Some(rx) = &self.root_watch_rx else {
            return false;
        };
        let mut changed = false;
        while let Ok(res) = rx.try_recv() {
            if let Ok(evt) = res {
                if matches!(
                    evt.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                ) {
                    changed = true;
                }
            }
        }
        changed
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

    /// Drains all pending config-watch events and returns `true` if the config
    /// file was modified, created, or deleted since the last check.
    pub(super) fn drain_config_watch(&self) -> bool {
        let (Some(rx), Some(watched)) = (&self.config_watch_rx, &self.config_path) else {
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
        // Capture whether this is a genuine file switch before we overwrite
        // current_file with the new path.
        let is_new_file = self.current_file.as_deref() != Some(path);

        self.viewing_revision = None;
        self.virtual_file = None;
        self.current_file = Some(path.to_path_buf());
        self.current_syntax = None;
        self.mark_session_dirty();
        self.plugin_content_active = false;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.is_diff = true;
        self.clear_fold_state();
        self.file_encoding = None;
        self.file_line_ending = None;
        if is_new_file {
            self.in_file_search = None;
            self.set_content_scroll(0);
            self.content_hscroll = 0;
            self.active_line = 0;
            self.show_line_blame = false;
            self.clear_selection();
            self.plugin_content_active_path = None;
        } else {
            self.clear_selection();
        }
        self.content_title = Some(load.content_title);
        self.highlighted = load.highlighted;
        self.diff_rows = load.diff_rows;
        self.content = load.content;
        if !is_new_file {
            // Same-file reload: clamp scroll and refresh in-file search.
            self.clamp_content_scroll();
            if self.in_file_search.is_some() {
                self.refresh_in_file_search();
            }
        }
        self.set_file_watch(Some(path));
        if !self.plugin_is_opening_file {
            self.plugin_manager.on_file_open(path);
        }
    }

    /// Shows a "[deleted]" placeholder for a file that was removed from the
    /// working tree but is tracked by git.
    pub(super) fn show_deleted(&mut self, path: &Path) {
        self.invalidate_pending_load();
        self.viewing_revision = None;
        self.in_file_search = None;
        self.current_file = Some(path.to_path_buf());
        self.current_syntax = None;
        self.mark_session_dirty();
        self.plugin_content_active = false;
        self.plugin_content_active_path = None;
        self.is_diff = false;
        self.diff_rows = Vec::new();
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
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
        let _span = tracing::info_span!("open_file").entered();
        self.invalidate_pending_load();
        let load = compute_file_load(
            path,
            &self.highlighter,
            self.config.content.prettify_size_limit,
        );
        self.apply_file_load(path, load);
    }

    /// Applies a computed file load to the content panel: resets scroll and
    /// selection, then installs the rendered content/highlighting/JSON/YAML
    /// state. Shared by the synchronous and worker-thread code paths.
    pub(super) fn apply_file_load(&mut self, path: &Path, load: FileLoad) {
        if load.ok {
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            let size_bucket = crate::telemetry::FileSizeBucket::from_size(size);
            let is_new_file = self.current_file.as_deref() != Some(path);
            let source_kind = if is_new_file {
                self.last_open_source
            } else {
                crate::telemetry::FileSourceKind::Reopen
            };
            let encoding = crate::telemetry::FileEncoding::from_str(load.encoding.as_deref());
            let is_binary = encoding == crate::telemetry::FileEncoding::Binary;
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::FileOpened {
                    size_bucket,
                    source_kind,
                    encoding,
                    is_binary,
                });
        }

        self.is_diff = false;
        self.viewing_revision = None;
        self.diff_rows = Vec::new();
        self.content_title = None;
        // Drop blame popup, active line, scroll, search only when navigating
        // to a different file; a same-file reopen (reload / external edit)
        // preserves all.
        let is_new_file = self.current_file.as_deref() != Some(path);
        if is_new_file {
            self.file_at_revision = None;
            self.viewing_revision_hash = None;
            // Remember outgoing cursor, restore incoming cursor
            if let Some(old) = self.current_file.clone() {
                self.cursor_positions
                    .insert(old, (self.active_line, self.content_scroll));
            }
            let (line, scroll) = self.cursor_positions.get(path).copied().unwrap_or((0, 0));
            self.in_file_search = None;
            // Assign raw: content not loaded yet, so content_scroll_max() is stale.
            // The `if is_new_file && load.ok` block below clamps once content is in place.
            self.content_scroll = scroll;
            self.content_hscroll = 0;
            self.active_line = line;
            self.show_line_blame = false;
            self.clear_selection();
            self.plugin_content_active_path = None;
        } else {
            self.clear_selection();
        }

        self.is_json = load.is_json;
        self.file_encoding = load.encoding;
        self.file_line_ending = load.line_ending;
        self.show_pretty_json = load.show_pretty_json;
        self.json_pretty_text = load.json_pretty_text;
        self.json_pretty_lines = load.json_pretty_lines;
        self.clear_fold_state();
        self.virtual_file = load.virtual_file;
        self.content = load.content;
        self.highlighted = load.highlighted;
        if let Some(y) = load.yaml {
            self.fold_regions = y.fold_regions;
            self.yaml_error = y.error;
            self.yaml_anchor_count = y.anchor_count;
            self.yaml_alias_count = y.alias_count;
        }
        // Language provider fold regions override built-in YAML regions.
        self.apply_plugin_fold_regions(path);

        if load.prettify_size_limit_exceeded {
            self.set_status("too large to pretty-print / fold");
        }

        // Clamp restored cursor to current content bounds.
        if is_new_file && load.ok {
            let max_line = self.display_line_count().saturating_sub(1);
            self.active_line = self.active_line.min(max_line);
            self.clamp_content_scroll();
        }

        if load.ok {
            self.current_file = Some(path.to_path_buf());
            self.current_syntax = load.syntax_name;
            self.mark_session_dirty();
            self.plugin_content_active = false;
            self.set_file_watch(Some(path));
            if !self.plugin_is_opening_file {
                self.plugin_manager.on_file_open(path);
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

    /// Adds `path` to the front of the recent-files ring, deduplicating
    /// and capping to `config.recent_files_count`.
    fn push_recent(&mut self, path: PathBuf) {
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
            self.last_open_source = crate::telemetry::FileSourceKind::RecentFiles;
            self.open_and_reveal(&path);
        }
    }

    /// Opens the global fuzzy file-name picker (SearchState in Files mode),
    /// scoped to git-changed files when in git mode. Focus-independent.
    pub fn open_file_search(&mut self) {
        let root = self.root.clone();
        let changed = self.git_changed_files_set();
        let mut s = SearchState::new(
            &root,
            self.show_hidden,
            self.ignore_gitignore,
            self.config.search.context_lines,
            changed.as_ref(),
        );
        if self.config.search.keep_query && !self.last_search_query.is_empty() {
            s.query = self.last_search_query.clone();
            s.refresh_now();
        }
        self.search = Some(s);
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
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::FeatureUsed {
                feature: crate::telemetry::Feature::GitHistory,
            });
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
            self.last_open_source = crate::telemetry::FileSourceKind::History;
            let diff = crate::git::file_diff(&self.root, &hash, &file);
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
        self.plugin_content_active = false;
        self.plugin_content_active_path = None;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
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
        self.highlighted = lines
            .iter()
            .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
            .collect();
        self.diff_rows = crate::diff::parse_side_by_side(lines);
        self.content = lines.to_vec();
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }

    /// Opens the repository-wide commit log as a picker overlay.
    /// Does nothing if not in a git repo.
    pub(super) fn open_repo_log(&mut self) {
        if self.git_info.is_none() {
            return;
        }
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::FeatureUsed {
                feature: crate::telemetry::Feature::GitHistory,
            });
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
            self.enter_compare_mode(hash);
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
                    self.content_scroll = saved.content_scroll;
                    self.active_line = saved.active_line;
                    self.diff_side_by_side = saved.side_by_side;
                    self.show_line_blame = false;
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
        self.show_blame = false;
        let diff = crate::git::file_diff(&self.root, &hash, &path);
        self.show_diff(&path, &short, &diff, Some(&hash));
        self.toggle_file_revision();
    }
}

#[cfg(test)]
#[path = "file_ops_test.rs"]
mod tests;
