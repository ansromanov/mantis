//! Command-palette dispatch and external-editor integration for `App`.
//!
//! `dispatch_command` maps a selected command-palette entry's `action_id` to the
//! shared `dispatch_action_id` path, which also serves the menu bar. The palette
//! and menu therefore share one set of canonical ids from
//! `crate::actions::ACTIONS`, avoiding `open_x`/`x_picker` aliasing between this
//! match and `Keymap::bindings_for_action`. This module
//! also owns suspending the TUI to launch the user's `$EDITOR` on the current
//! file: it tears down raw mode and the alternate screen, runs the editor,
//! then restores the terminal and flags `needs_clear` so the next frame
//! repaints cleanly. Theme switching and the highlighter rebuild triggered
//! from the palette live here too, alongside the related terminal-state
//! bookkeeping. `open_release_url` delegates to the shared `open_in_browser`
//! helper, which guards against spawning the browser when stdout is not a TTY
//! (piped/headless/CI runs). `open_external` guards the same way and shares
//! the OS-dispatch spawn logic with `open_in_browser` via `spawn_system_open`.

use crate::search::{GotoLineState, SearchState, ThemePicker};

use super::super::{App, Focus};

impl App {
    /// Executes the selected command from the palette and closes it. Returns
    /// whether `action_id` matched a known dispatch arm - not whether that
    /// arm's own state-dependent guard (e.g. `is_diff`) actually fired an
    /// effect. `actions_test.rs::every_palette_action_id_is_dispatch_handled`
    /// calls this directly so a removed match arm fails that test instead of
    /// leaving a hand-maintained id list stale.
    pub(crate) fn dispatch_command(&mut self) -> bool {
        let action_id = self
            .command_palette
            .as_ref()
            .and_then(|p| p.selected_command().map(|c| c.action_id.clone()));
        match action_id {
            Some(id) => self.dispatch_action_id(&id),
            None => {
                self.command_palette = None;
                false
            }
        }
    }

    /// Dispatches a canonical registry action from a non-palette surface.
    /// Applicability, usage recording, and telemetry stay shared with the
    /// command palette so menu items have identical behavior.
    pub(crate) fn dispatch_action_id(&mut self, id: &str) -> bool {
        if let Err(reason) = self.check_applicability(id) {
            let name = crate::actions::ACTIONS
                .iter()
                .find(|action| action.id == id)
                .and_then(|action| action.palette)
                .unwrap_or(id);
            self.set_status(format!("{}: {}", name, reason));
            self.command_palette = None;
            return true;
        }
        // Usage stats and telemetry record canonical action ids only;
        // other ids are not in ACTIONS.
        if let Some(spec) = crate::actions::ACTIONS
            .iter()
            .find(|action| action.id == id)
        {
            self.command_usage.record(spec.id);
            self.command_usage.save();
        }
        self.command_palette = None;
        match Some(id) {
            Some("context_menu") => {
                self.open_focused_context_menu();
                true
            }
            Some("help") => {
                self.show_help = !self.show_help;
                true
            }
            Some("menu_bar") => {
                self.open_menu_bar();
                true
            }
            Some("bug_report") => {
                let report = crate::diagnostics::DiagnosticReport::collect(self);
                self.bug_report = Some(crate::search::BugReportState::new(report.to_markdown()));
                true
            }
            Some("quit") => {
                self.should_quit = true;
                true
            }
            Some("toggle_hidden") => {
                self.show_hidden = !self.show_hidden;
                self.config.tree.show_hidden = self.show_hidden;
                self.reload();
                self.save_config();
                true
            }
            Some("search_files") => {
                self.open_file_search();
                true
            }
            Some("find_files") => {
                self.open_file_search();
                true
            }
            Some("search_content") => {
                let root = self.root.clone();
                let changed = self.git_changed_files_set();
                let mut s = SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search.context_lines,
                    changed.as_ref(),
                );
                s.toggle_mode();
                if self.config.search.keep_query && !self.last_search_query.is_empty() {
                    s.query = self.last_search_query.clone();
                    s.refresh_now();
                }
                self.search = Some(s);
                true
            }
            Some("reload") => {
                self.reload();
                true
            }
            Some("file_history") => {
                self.open_file_history();
                true
            }
            Some("repo_commit_log") => {
                self.open_repo_log();
                true
            }
            Some("theme_picker") => {
                self.theme_picker = Some(ThemePicker::default());
                true
            }
            Some("new_tab") => {
                self.tab_action_request = Some(super::super::TabAction::New);
                true
            }
            Some("close_tab") => {
                self.tab_action_request = Some(super::super::TabAction::Close);
                true
            }
            Some("next_tab") => {
                self.tab_action_request = Some(super::super::TabAction::Next);
                true
            }
            Some("prev_tab") => {
                self.tab_action_request = Some(super::super::TabAction::Prev);
                true
            }
            Some("move_tab_prev") => {
                self.tab_action_request = Some(super::super::TabAction::MovePrev);
                true
            }
            Some("move_tab_next") => {
                self.tab_action_request = Some(super::super::TabAction::MoveNext);
                true
            }
            Some("tab_picker") => {
                self.tab_action_request = Some(super::super::TabAction::Picker);
                true
            }
            Some("reopen_tab") => {
                self.tab_action_request = Some(super::super::TabAction::Reopen);
                true
            }
            Some("git_mode_toggle") => {
                self.toggle_git_mode();
                true
            }
            Some("git_mode_flat_toggle") => {
                if self.git_mode {
                    self.git_mode_flat = !self.git_mode_flat;
                    self.rebuild(true);
                    self.try_open_selected();
                    self.save_config();
                }
                true
            }
            Some("toggle_wrap") => {
                self.toggle_word_wrap();
                true
            }
            Some("toggle_line_numbers") => {
                self.show_line_numbers = !self.show_line_numbers;
                self.config.content.line_numbers = self.show_line_numbers;
                self.save_config();
                true
            }
            Some("toggle_pretty_json") => {
                if self.is_json && !self.json_pretty_lines.is_empty() {
                    let active_line = self.active_line;
                    let scroll = self.content_scroll;
                    self.show_pretty_json = !self.show_pretty_json;
                    self.content_hscroll = 0;
                    self.active_line = active_line.min(self.display_line_count().saturating_sub(1));
                    self.clamp_content_scroll();
                    // Clamp against the new representation, then restore the
                    // old viewport offset (which may have been illegal for
                    // the representation that was active before the toggle).
                    self.set_content_scroll(scroll);
                }
                true
            }
            Some("toggle_table_view") => {
                if self.is_csv && !self.csv_table_lines.is_empty() {
                    self.show_csv_table = !self.show_csv_table;
                    self.content_hscroll = 0;
                    self.clamp_content_scroll();
                }
                true
            }
            Some("toggle_secret_reveal") => {
                if self.secret_original.is_empty() || !self.secret_masked {
                    self.set_status("secret masking: no detected credentials");
                } else {
                    self.secret_revealed = !self.secret_revealed;
                    self.content = if self.secret_revealed {
                        self.secret_original.clone()
                    } else {
                        self.secret_original
                            .iter()
                            .map(|line| crate::secret_mask::mask_line(line))
                            .collect()
                    };
                    self.highlighted = self.highlighter.highlight(
                        &self.current_file.clone().unwrap_or_default(),
                        &self.content,
                    );
                    self.set_status(if self.secret_revealed {
                        "secrets revealed"
                    } else {
                        "secrets masked"
                    });
                }
                true
            }
            Some("json_query") => {
                self.open_json_query();
                true
            }
            Some("toggle_blame") => {
                if self.has_text_cursor() {
                    self.show_blame = !self.show_blame;
                } else {
                    self.set_status("blame: not available in a diff");
                }
                true
            }
            Some("toggle_diff_side_by_side") => {
                if self.is_diff {
                    self.diff_side_by_side = !self.diff_side_by_side;
                    self.config.git.diff.side_by_side = self.diff_side_by_side;
                    self.save_config();
                    self.content_hscroll = 0;
                    self.clamp_content_scroll();
                }
                true
            }
            Some("toggle_diff_staged") => {
                if self.is_diff {
                    self.diff_mode = self.diff_mode.next();
                    self.config.git.diff.mode = self.diff_mode;
                    self.save_config();
                    if let Some(path) = self.current_file.clone() {
                        self.show_working_tree_diff(&path);
                    }
                }
                true
            }
            Some("diff_hunk_next") => {
                if self.is_diff {
                    self.diff_next_hunk();
                }
                true
            }
            Some("diff_hunk_prev") => {
                if self.is_diff {
                    self.diff_prev_hunk();
                }
                true
            }
            Some("open_in_editor") => {
                self.open_in_editor();
                true
            }
            Some("open_external") => {
                self.open_external_file();
                true
            }
            Some("open_config_in_editor") => {
                self.open_config_in_editor();
                true
            }
            Some("show_about") => {
                self.show_about = !self.show_about;
                true
            }
            Some("fold_all") => {
                if !self.fold_regions.is_empty() {
                    self.fold_all();
                    self.mark_content_scrolled();
                }
                true
            }
            Some("unfold_all") => {
                if !self.fold_regions.is_empty() {
                    self.unfold_all();
                    self.mark_content_scrolled();
                }
                true
            }
            Some("fold_toggle") => {
                if self.is_jsonl && self.has_text_cursor() {
                    let source_line = self.display_to_physical(self.active_line);
                    if !self.jsonl_expanded.remove(&source_line) {
                        self.jsonl_expanded.insert(source_line);
                    }
                    self.rebuild_jsonl_display();
                    self.active_line = self
                        .physical_to_display(source_line)
                        .min(self.display_line_count().saturating_sub(1));
                    self.clamp_content_scroll();
                    return true;
                }
                if !self.fold_regions.is_empty() {
                    let phys = self.display_to_physical(self.content_scroll);
                    if let Some(ri) = self.region_idx_at(phys) {
                        self.toggle_fold_region(ri);
                        self.mark_content_scrolled();
                    }
                }
                true
            }
            Some("blame_line") => {
                if self.has_text_cursor() {
                    self.show_line_blame = !self.show_line_blame;
                }
                true
            }
            Some("follow_tail") => {
                self.toggle_follow_mode();
                true
            }
            Some("filter_lines") => {
                self.filter_bar = Some(crate::search::FilterBarState::new());
                true
            }
            Some("copy_path") => {
                self.copy_path_to_clipboard(false);
                true
            }
            Some("copy_relative_path") => {
                self.copy_path_to_clipboard(true);
                true
            }
            Some("copy_line") => {
                self.copy_line_or_selection();
                true
            }
            Some("copy_file") => {
                self.copy_file_content();
                true
            }
            Some("tree_collapse_all") => {
                self.collapse_all();
                true
            }
            Some("tree_expand_all") => {
                self.expand_all();
                true
            }
            Some("tree_up_dir") => {
                self.tree_up_dir();
                true
            }
            Some("tree_width_grow") => {
                self.tree_width = (self.tree_width + 2).min(95);
                self.config.tree.width = self.tree_width;
                self.save_config();
                true
            }
            Some("tree_width_shrink") => {
                self.tree_width = self.tree_width.saturating_sub(2).max(5);
                self.config.tree.width = self.tree_width;
                self.save_config();
                true
            }
            Some("recent_files") => {
                self.open_recent_files();
                true
            }
            Some("toggle_bookmark") => {
                self.toggle_bookmark();
                true
            }
            Some("bookmarks") => {
                self.open_bookmarks();
                true
            }
            Some("worktree_picker") => {
                self.worktree_picker = Some(crate::search::WorktreePicker::new(&self.root));
                true
            }
            Some("open_worktree_new_tab") => {
                self.worktree_picker = Some(crate::search::WorktreePicker::new_tab(&self.root));
                true
            }
            Some("toggle_watch") => {
                self.auto_watch = !self.auto_watch;
                self.config.content.watch = self.auto_watch;
                self.save_config();
                true
            }
            Some("goto_line") => {
                if self.focus == Focus::Content {
                    self.goto_line = Some(GotoLineState::new());
                }
                true
            }
            Some("symbol_outline") => {
                self.open_symbol_outline();
                true
            }
            Some("diagnostics_picker") => {
                self.open_diagnostics_picker();
                true
            }
            Some("compare_against") => {
                self.revision_picker = Some(crate::search::RevisionPicker::new(&self.root));
                true
            }
            Some("toggle_file_revision") => {
                self.toggle_file_revision();
                true
            }
            Some("blame_open_commit") => {
                if !self.show_blame {
                    self.set_status("blame open commit: blame view not active");
                    return true;
                }
                self.open_blame_commit_at_active_line();
                true
            }
            Some("toggle_raw_markdown") => {
                self.toggle_raw_markdown();
                true
            }
            _ => false,
        }
    }
}
