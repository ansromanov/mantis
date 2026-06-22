//! Command-palette dispatch and external-editor integration for `App`.
//!
//! `dispatch_command` maps a selected command-palette entry's `action_id` to the
//! matching `App` method, so the palette and direct keybindings share one set of
//! actions. This module also owns suspending the TUI to launch the user's
//! `$EDITOR` on the current file: it tears down raw mode and the alternate
//! screen, runs the editor, then restores the terminal and flags `needs_clear`
//! so the next frame repaints cleanly. Theme switching and the highlighter
//! rebuild triggered from the palette live here too, alongside the related
//! terminal-state bookkeeping.

use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};

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
        self.command_palette = None;
        match action_id {
            Some("toggle_help") => self.show_help = !self.show_help,
            Some("toggle_hidden") => {
                self.show_hidden = !self.show_hidden;
                self.config.show_hidden = self.show_hidden;
                self.reload();
                self.save_config();
            }
            Some("open_file_search") => {
                let root = self.root.clone();
                let mut s = SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search_context_lines,
                );
                if self.config.keep_search_query && !self.last_search_query.is_empty() {
                    s.query = self.last_search_query.clone();
                    s.refresh_now();
                }
                self.search = Some(s);
            }
            Some("open_content_search") => {
                let root = self.root.clone();
                let mut s = SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search_context_lines,
                );
                s.toggle_mode();
                if self.config.keep_search_query && !self.last_search_query.is_empty() {
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
            Some("toggle_git_flat") => {
                if self.git_mode {
                    self.git_mode_flat = !self.git_mode_flat;
                    self.config.git_mode_flat = self.git_mode_flat;
                    self.rebuild();
                    self.try_open_selected();
                    self.save_config();
                }
            }
            Some("toggle_word_wrap") => {
                self.word_wrap = !self.word_wrap;
                self.config.word_wrap = self.word_wrap;
                self.content_scroll = 0;
                self.content_hscroll = 0;
                self.save_config();
            }
            Some("toggle_line_numbers") => {
                self.show_line_numbers = !self.show_line_numbers;
                self.config.line_numbers = self.show_line_numbers;
                self.save_config();
            }
            Some("toggle_raw_markdown") if self.is_markdown => {
                self.show_raw_markdown = !self.show_raw_markdown;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            }
            Some("toggle_pretty_json") if self.is_json && !self.json_pretty_lines.is_empty() => {
                self.show_pretty_json = !self.show_pretty_json;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            }
            Some("toggle_diff_side_by_side") if self.is_diff => {
                self.diff_side_by_side = !self.diff_side_by_side;
                self.content_scroll = 0;
                self.content_hscroll = 0;
            }
            Some("toggle_visual_line") if !self.is_diff && self.current_file.is_some() => {
                self.focus = Focus::Content;
                self.enter_visual_line();
            }
            Some("open_in_editor") => self.open_in_editor(),
            Some("open_config_in_editor") => self.open_config_in_editor(),
            Some("show_about") => self.show_about = !self.show_about,
            Some("yaml_fold_all") if !self.yaml_fold_regions.is_empty() => {
                self.fold_all();
                self.mark_content_scrolled();
            }
            Some("yaml_unfold_all") if !self.yaml_fold_regions.is_empty() => {
                self.unfold_all();
                self.mark_content_scrolled();
            }
            Some("yaml_fold_toggle") if !self.yaml_fold_regions.is_empty() => {
                let phys = self.display_to_physical(self.content_scroll);
                if let Some(ri) = self.region_idx_at(phys) {
                    self.toggle_fold_region(ri);
                    self.mark_content_scrolled();
                }
            }
            Some("blame_line") => {
                if !self.is_diff {
                    self.show_line_blame = !self.show_line_blame;
                }
            }
            Some("copy_path") => self.copy_path_to_clipboard(false),
            Some("copy_relative_path") => self.copy_path_to_clipboard(true),
            Some("tree_collapse_all") => self.collapse_all(),
            Some("tree_expand_all") => self.expand_all(),
            Some("go_to_line") if self.focus == Focus::Content => {
                self.goto_line = Some(GotoLineState::new());
            }
            _ => {}
        }
    }

    pub(super) fn open_release_url(&self) {
        let Some(release) = crate::release_info::RELEASE.as_ref() else {
            return;
        };
        let url = release.release_url.clone();
        if url.is_empty() {
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &url])
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
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
    fn apply_theme(&mut self, theme_name: &str, theme: Theme) {
        // Notify plugins so they can re-render content with matching colours.
        self.plugin_manager.on_theme_change(theme_name);
        self.plugin_content.clear();
        self.theme = theme;
        self.highlighter =
            Highlighter::with_extra_syntaxes(&self.theme.syntax, &self.extra_syntaxes);
        self.loader_set_theme();
        // Notify plugins so they can re-render with matching colours.
        // Use the config theme name if available, falling back to "default".
        let theme_name = self
            .config
            .theme
            .name
            .clone()
            .unwrap_or_else(|| "default".to_string());
        self.plugin_manager.on_theme_change(&theme_name);
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

    /// Opens the currently selected file in the user's `$EDITOR` (falling back
    /// to `$VISUAL`). Suspends the TUI, spawns the editor, waits for it to
    /// exit, then restores the TUI and reloads the file content.
    pub(super) fn open_in_editor(&mut self) {
        let Some(path) = self.current_file.clone() else {
            return;
        };

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| {
                if cfg!(windows) {
                    "notepad".to_string()
                } else {
                    "vim".to_string()
                }
            });

        // Suspend TUI
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture,);

        // Spawn editor and wait for it to finish.
        // Split on whitespace so $EDITOR="code --wait" works correctly.
        let parts: Vec<&str> = editor.split_whitespace().collect();
        if let Some((cmd, args)) = parts.split_first() {
            let _ = std::process::Command::new(cmd)
                .args(args)
                .arg(&path)
                .status();
        }

        // Restore TUI
        let _ = execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture,);
        if let Err(e) = enable_raw_mode() {
            eprintln!("tv: failed to restore raw mode after editor: {e}");
        }

        // Flag that the terminal was suspended so main.rs clears ratatui's
        // internal buffer (which is stale after re-entering the alt screen).
        self.needs_clear = true;

        // File may have been modified; reload its content
        self.reload_content();
    }

    /// Opens the global config file (`~/.config/tree-viewer/tv.toml`) in the
    /// user's `$EDITOR`, using the same suspend/resume pattern as `open_in_editor`.
    fn open_config_in_editor(&mut self) {
        let Some(path) = self.config_path.clone() else {
            return;
        };

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| {
                if cfg!(windows) {
                    "notepad".to_string()
                } else {
                    "vim".to_string()
                }
            });

        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture,);

        let parts: Vec<&str> = editor.split_whitespace().collect();
        if let Some((cmd, args)) = parts.split_first() {
            let _ = std::process::Command::new(cmd)
                .args(args)
                .arg(&path)
                .status();
        }

        let _ = execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture,);
        if let Err(e) = enable_raw_mode() {
            eprintln!("tv: failed to restore raw mode after editor: {e}");
        }

        self.needs_clear = true;
        self.reload_config();
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
        let Ok(cfg) = toml::from_str::<config::Config>(&s) else {
            return;
        };

        self.show_hidden = cfg.show_hidden;
        self.ignore_gitignore = cfg.ignore_gitignore;
        self.tree_width = cfg.tree_width;
        self.tree_independent_scroll = cfg.tree_independent_scroll;
        self.word_wrap = cfg.word_wrap;
        self.git_status_enabled = cfg.git_status || cfg.git_mode;
        self.git_show_deleted = cfg.git_show_deleted;
        self.git_mode = cfg.git_mode;
        self.git_mode_flat = cfg.git_mode_flat;
        self.show_scrollbar = cfg.scrollbar;
        self.show_scroll_percentage = cfg.scroll_percentage;
        self.keys = cfg.keys.clone();
        self.icons_enabled = cfg.icons;

        let theme_name = cfg.theme.name.as_deref().unwrap_or("default").to_string();
        let theme = cfg.theme.resolve();
        self.apply_theme(&theme_name, theme);

        self.config = cfg;
        self.reload();
    }
}

#[cfg(test)]
#[path = "editor_test.rs"]
mod tests;
