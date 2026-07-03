//! Command-palette dispatch and external-editor integration for `App`.
//!
//! `dispatch_command` maps a selected command-palette entry's `action_id` to the
//! matching `App` method, so the palette and direct keybindings share one set of
//! actions. This module also owns suspending the TUI to launch the user's
//! `$EDITOR` on the current file: it tears down raw mode and the alternate
//! screen, runs the editor, then restores the terminal and flags `needs_clear`
//! so the next frame repaints cleanly. Theme switching and the highlighter
//! rebuild triggered from the palette live here too, alongside the related
//! terminal-state bookkeeping. `open_release_url` delegates to the shared
//! `open_in_browser` helper, which guards against spawning the browser when
//! stdout is not a TTY (piped/headless/CI runs).

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

use std::io::IsTerminal;
use std::path::Path;

use crate::config;
use crate::highlight::Highlighter;
use crate::search::{GotoLineState, PluginPicker, SearchState, ThemePicker};
use crate::theme::{Theme, ThemeConfig};

use super::super::{diff_line_style, App, Focus};

impl App {
    /// Executes the selected command from the palette and closes it.
    pub(crate) fn dispatch_command(&mut self) {
        let action_id = self
            .command_palette
            .as_ref()
            .and_then(|p| p.selected_command().map(|c| c.action_id));
        if let Some(id) = action_id {
            self.command_usage.record(id);
            self.command_usage.save();
        }
        self.command_palette = None;
        match action_id {
            Some("toggle_help") => self.show_help = !self.show_help,
            Some("toggle_hidden") => {
                self.show_hidden = !self.show_hidden;
                self.config.tree.show_hidden = self.show_hidden;
                self.reload();
                self.save_config();
            }
            Some("open_file_search") => self.open_file_search(),
            Some("open_content_search") => {
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
            }
            Some("reload") => self.reload(),
            Some("open_file_history") => self.open_file_history(),
            Some("open_theme_picker") => {
                self.theme_picker = Some(ThemePicker::default());
            }
            Some("open_plugin_picker") => {
                let entries = self.plugin_manager.plugin_entries();
                self.plugin_picker = Some(PluginPicker::new(entries));
            }
            Some("toggle_git_mode") => self.toggle_git_mode(),
            Some("toggle_git_flat") if self.git_mode => {
                self.git_mode_flat = !self.git_mode_flat;
                self.rebuild(true);
                self.try_open_selected();
                self.save_config();
            }
            Some("toggle_word_wrap") => {
                self.word_wrap = !self.word_wrap;
                self.config.content.word_wrap = self.word_wrap;
                self.set_content_scroll(0);
                self.content_hscroll = 0;
                self.save_config();
            }
            Some("toggle_line_numbers") => {
                self.show_line_numbers = !self.show_line_numbers;
                self.config.content.line_numbers = self.show_line_numbers;
                self.save_config();
            }
            Some("toggle_raw_markdown") if self.is_markdown => {
                self.show_raw_markdown = !self.show_raw_markdown;
                self.set_content_scroll(0);
                self.content_hscroll = 0;
            }
            Some("toggle_pretty_json") if self.is_json && !self.json_pretty_lines.is_empty() => {
                self.show_pretty_json = !self.show_pretty_json;
                self.set_content_scroll(0);
                self.content_hscroll = 0;
            }
            Some("toggle_diff_side_by_side") if self.is_diff => {
                self.diff_side_by_side = !self.diff_side_by_side;
                self.config.git.diff.side_by_side = self.diff_side_by_side;
                self.save_config();
                self.set_content_scroll(0);
                self.content_hscroll = 0;
            }
            Some("open_in_editor") => self.open_in_editor(),
            Some("open_config_in_editor") => self.open_config_in_editor(),
            Some("show_about") => self.show_about = !self.show_about,
            Some("fold_all") if !self.fold_regions.is_empty() => {
                self.fold_all();
                self.mark_content_scrolled();
            }
            Some("unfold_all") if !self.fold_regions.is_empty() => {
                self.unfold_all();
                self.mark_content_scrolled();
            }
            Some("fold_toggle") if !self.fold_regions.is_empty() => {
                let phys = self.display_to_physical(self.content_scroll);
                if let Some(ri) = self.region_idx_at(phys) {
                    self.toggle_fold_region(ri);
                    self.mark_content_scrolled();
                }
            }
            Some("blame_line") if self.has_text_cursor() => {
                self.show_line_blame = !self.show_line_blame;
            }
            Some("copy_path") => self.copy_path_to_clipboard(false),
            Some("copy_relative_path") => self.copy_path_to_clipboard(true),
            Some("tree_collapse_all") => self.collapse_all(),
            Some("tree_expand_all") => self.expand_all(),
            Some("tree_up_dir") => self.tree_up_dir(),
            Some("go_to_line") if self.focus == Focus::Content => {
                self.goto_line = Some(GotoLineState::new());
            }
            _ => {}
        }
    }

    pub(super) fn open_release_url(&mut self) {
        let Some(release) = crate::release_info::RELEASE.as_ref() else {
            return;
        };
        self.open_in_browser(&release.release_url);
    }
}

/// Resolves the external editor command from environment, falling back to
/// an OS-appropriate default (`vim` on Unix, `notepad` on Windows).
pub(super) fn resolve_editor() -> String {
    std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vim".to_string()
            }
        })
}

impl App {
    /// Suspends the TUI, opens `path` in the user's `$EDITOR` (resolved by
    /// `resolve_editor`), waits for the editor to exit, restores the TUI,
    /// and flags a clear. The caller is responsible for reloading content
    /// after the editor returns.
    fn launch_editor(&mut self, path: &Path) {
        let editor = resolve_editor();

        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);

        let parts: Vec<&str> = editor.split_whitespace().collect();
        let launch_err = if let Some((cmd, args)) = parts.split_first() {
            std::process::Command::new(cmd)
                .args(args)
                .arg(path)
                .status()
                .err()
        } else {
            None
        };

        let _ = execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture);
        if let Err(e) = enable_raw_mode() {
            eprintln!("mantis: failed to restore raw mode after editor: {e}");
        }
        self.needs_clear = true;
        if let Some(e) = launch_err {
            self.set_status(format!("editor launch failed: {e}"));
        }
    }

    /// Opens `url` in the system browser. No-op for empty URLs. When stdout
    /// is not a terminal (piped/headless/CI), sets a status message instead
    /// of spawning the browser.
    pub(super) fn open_in_browser(&mut self, url: &str) {
        if url.is_empty() {
            return;
        }
        if !std::io::stdout().is_terminal() {
            self.set_status("not opening browser (non-interactive)");
            return;
        }
        #[cfg(target_os = "macos")]
        if let Err(e) = std::process::Command::new("open").arg(url).spawn() {
            self.set_status(format!("browser launch failed: {e}"));
        }
        #[cfg(target_os = "windows")]
        if let Err(e) = std::process::Command::new("cmd")
            .args(["/c", "start", "", url])
            .spawn()
        {
            self.set_status(format!("browser launch failed: {e}"));
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        if let Err(e) = std::process::Command::new("xdg-open").arg(url).spawn() {
            self.set_status(format!("browser launch failed: {e}"));
        }
    }

    /// Applies the theme selected in the picker, saves it to config, and
    /// closes the overlay.
    pub(crate) fn apply_selected_theme(&mut self) {
        let name = self
            .theme_picker
            .as_ref()
            .and_then(|p| p.selected_name())
            .map(String::from);
        self.theme_picker = None;
        if let Some(ref name) = name {
            if let Some(theme) = Theme::load(name) {
                self.apply_theme(name, theme);
                self.config.theme = ThemeConfig::from_preset(name);
                self.save_config();
            }
        }
    }

    /// Switches the active theme and re-renders the current view with it,
    /// preserving scroll position.
    pub(crate) fn apply_theme(&mut self, theme_name: &str, theme: Theme) {
        // Notify plugins so they can re-render content with matching colours.
        self.plugin_manager.on_theme_change(theme_name);
        self.plugin_content.clear();
        self.plugin_content_text.clear();
        self.theme = theme;
        self.highlighter =
            Highlighter::with_extra_syntaxes(&self.theme.syntax, &self.extra_syntaxes);
        self.loader_set_theme();
        if self.is_diff {
            self.highlighted = self
                .content
                .iter()
                .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
                .collect();
        } else if let Some(path) = self.current_file.clone() {
            self.reopen_file(&path);
        }
    }

    /// Opens the currently selected file in the user's `$EDITOR`. Delegates
    /// to `launch_editor` for the TUI suspend/resume dance, then reloads the
    /// file content.
    pub(super) fn open_in_editor(&mut self) {
        if let Some(p) = self.current_file.clone() {
            self.launch_editor(&p);
            self.reload_content();
        }
    }

    /// Opens the config file in the user's `$EDITOR`. Delegates to
    /// `launch_editor` for the TUI suspend/resume dance, then re-reads config.
    fn open_config_in_editor(&mut self) {
        if let Some(p) = self.config_path.clone() {
            self.launch_editor(&p);
            self.reload_config();
        }
    }

    /// Re-reads the config file and applies all changed fields to App state,
    /// then rebuilds the tree and reloads the current file. Silently ignores
    /// read or parse errors so a mid-edit save doesn't crash the app.
    fn reload_config(&mut self) {
        let Some(path) = self.config_path.clone() else {
            return;
        };
        let Ok(s) = std::fs::read_to_string(&path) else {
            return;
        };
        let Ok(mut cfg) = toml::from_str::<config::Config>(&s) else {
            return;
        };
        cfg.migrate_legacy_flat_fields();
        cfg.migrate_legacy_git_fields();

        self.show_hidden = cfg.tree.show_hidden;
        self.ignore_gitignore = cfg.git.ignore_gitignore;
        self.tree_width = cfg.tree.width;
        self.tree_independent_scroll = cfg.tree.independent_scroll;
        self.word_wrap = cfg.content.word_wrap;
        self.git_status_enabled = cfg.git.status;
        self.git_show_deleted = cfg.git.show_deleted;
        self.git_show_untracked = cfg.git.show_untracked;
        self.git_show_ignored = cfg.git.show_ignored;
        self.show_scrollbar = cfg.content.scrollbar;
        self.show_scroll_percentage = cfg.content.scroll_percentage;
        self.keys = cfg.keys.clone();
        self.icons_enabled = cfg.tree.icons;

        let theme_name = cfg.theme.name.as_deref().unwrap_or("default").to_string();
        let theme = cfg.theme.resolve();
        self.apply_theme(&theme_name, theme);

        self.config = cfg;
        self.reload();
    }
}
