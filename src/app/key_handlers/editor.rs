//! Command-palette dispatch and external-editor integration for `App`.
//!
//! `dispatch_command` maps a selected command-palette entry's `action_id` to the
//! matching `App` method, so the palette and direct keybindings share one set of
//! canonical ids from `crate::actions::ACTIONS` - no more `open_x`/`x_picker`
//! aliasing between this match and `Keymap::bindings_for_action`. This module
//! also owns suspending the TUI to launch the user's `$EDITOR` on the current
//! file: it tears down raw mode and the alternate screen, runs the editor,
//! then restores the terminal and flags `needs_clear` so the next frame
//! repaints cleanly. Theme switching and the highlighter rebuild triggered
//! from the palette live here too, alongside the related terminal-state
//! bookkeeping. `open_release_url` delegates to the shared `open_in_browser`
//! helper, which guards against spawning the browser when stdout is not a TTY
//! (piped/headless/CI runs). `open_external` guards the same way and shares
//! the OS-dispatch spawn logic with `open_in_browser` via `spawn_system_open`.

use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};

use std::io::IsTerminal;
use std::path::Path;

use crate::highlight::Highlighter;
use crate::search::{GotoLineState, PluginPicker, SearchState, ThemePicker};
use crate::theme::{Theme, ThemeConfig};

use super::super::{diff_line_style, App, Focus};

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
            .and_then(|p| p.selected_command().map(|c| c.action_id));
        if let Some(id) = action_id {
            if let Err(reason) = self.check_applicability(id) {
                let name = crate::command_palette::COMMANDS
                    .iter()
                    .find(|c| c.action_id == id)
                    .map(|c| c.name)
                    .unwrap_or(id);
                self.set_status(format!("{}: {}", name, reason));
                self.command_palette = None;
                return true;
            }
            self.command_usage.record(id);
            self.command_usage.save();
            self.telemetry
                .record(crate::telemetry::TelemetryEvent::ActionInvoked {
                    action: id,
                    source: crate::telemetry::ActionSource::Palette,
                });
        }
        self.command_palette = None;
        match action_id {
            Some("help") => {
                self.show_help = !self.show_help;
                true
            }
            Some("bug_report") => {
                let report = crate::diagnostics::DiagnosticReport::collect(self);
                self.bug_report = Some(crate::search::BugReportState::new(report.to_markdown()));
                true
            }
            Some("toggle_telemetry") => {
                self.toggle_telemetry();
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
            Some("theme_picker") => {
                self.theme_picker = Some(ThemePicker::default());
                true
            }
            Some("plugin_picker") => {
                let entries = self.plugin_manager.plugin_entries();
                self.plugin_picker = Some(PluginPicker::new(entries));
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
                self.word_wrap = !self.word_wrap;
                self.config.content.word_wrap = self.word_wrap;
                self.set_content_scroll(0);
                self.content_hscroll = 0;
                self.save_config();
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
                    self.show_pretty_json = !self.show_pretty_json;
                    self.set_content_scroll(0);
                    self.content_hscroll = 0;
                }
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
                    self.set_content_scroll(0);
                    self.content_hscroll = 0;
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
            Some("recent_files") => {
                self.open_recent_files();
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
            Some("compare_against") => {
                self.revision_picker = Some(crate::search::RevisionPicker::new(&self.root));
                true
            }
            Some("toggle_raw_markdown") => {
                if self.plugin_content_active {
                    let key = crossterm::event::KeyEvent::new(
                        crossterm::event::KeyCode::Char('M'),
                        crossterm::event::KeyModifiers::SHIFT,
                    );
                    self.plugin_manager.on_keypress(&key);
                } else {
                    self.set_status(
                        "markdown render toggle: not available (current file not plugin-rendered)",
                    );
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn open_release_url(&mut self) {
        let Some(release) = crate::release_info::RELEASE.as_ref() else {
            return;
        };
        if let Err(e) = self.open_in_browser(&release.release_url) {
            self.set_status(e);
        }
    }
}

/// Resolves the external editor command from config, environment, or a
/// platform-appropriate default.
///
/// Priority: config editor > `$VISUAL` > `$EDITOR` > probe for `nano` >
/// `vim` (Unix) / `notepad` (Windows). Returns the command string and a
/// boolean indicating whether the fallback was used (no config or env var).
pub(super) fn resolve_editor(config_editor: Option<&str>) -> (String, bool) {
    // 1. Config override takes precedence
    if let Some(editor) = config_editor {
        if !editor.is_empty() {
            return (editor.to_string(), false);
        }
    }
    // 2. Environment variables
    if let Ok(editor) = std::env::var("VISUAL") {
        if !editor.is_empty() {
            return (editor, false);
        }
    }
    if let Ok(editor) = std::env::var("EDITOR") {
        if !editor.is_empty() {
            return (editor, false);
        }
    }
    // 3. Platform-appropriate fallback
    if cfg!(windows) {
        return ("notepad".to_string(), true);
    }
    if is_executable_in_path("nano") {
        ("nano".to_string(), true)
    } else {
        ("vim".to_string(), true)
    }
}

/// Returns `true` when `name` resolves to an executable on `$PATH`.
fn is_executable_in_path(name: &str) -> bool {
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new("which")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl App {
    /// Suspends the TUI, opens `path` in the user's `$EDITOR` (resolved by
    /// `resolve_editor`), waits for the editor to exit, restores the TUI,
    /// and flags a clear. The caller is responsible for reloading content
    /// after the editor returns.
    fn launch_editor(&mut self, path: &Path) {
        let config_editor = self.config.general.editor.as_deref();
        let (editor, is_fallback) = resolve_editor(config_editor);

        crate::app::restore_terminal();

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
        crate::app::set_alternate_scroll(false);
        if let Err(e) = enable_raw_mode() {
            eprintln!("mantis: failed to restore raw mode after editor: {e}");
        }
        self.needs_clear = true;
        if let Some(e) = launch_err {
            self.set_status(format!("editor launch failed: {e}"));
        } else if is_fallback {
            let editor_name = editor.split_whitespace().next().unwrap_or(&editor);
            self.set_status(format!(
                "Opened with {editor_name} — set $EDITOR to choose your editor"
            ));
        }
    }

    /// Opens `url` in the system browser. No-op for empty URLs. When stdout
    /// is not a terminal (piped/headless/CI), returns an error instead
    /// of spawning the browser.
    pub(crate) fn open_in_browser(&mut self, url: &str) -> Result<(), String> {
        if url.is_empty() {
            return Ok(());
        }
        if !std::io::stdout().is_terminal() {
            return Err("not opening browser (non-interactive)".to_string());
        }
        match spawn_system_open(url.as_ref()) {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("browser launch failed: {e}")),
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
        self.plugin_manager.on_theme_change(theme_name, &theme);
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
            self.handle_config_change();
        }
    }
    /// Opens the currently selected file in the system default application.
    pub(super) fn open_external_file(&mut self) {
        if let Some(p) = self.current_file.clone() {
            self.open_external(&p);
        }
    }

    /// Opens `path` in the system default application.
    /// When stdout is not a terminal (piped/headless/CI), sets a status message instead.
    pub(super) fn open_external(&mut self, path: &Path) {
        if !std::io::stdout().is_terminal() {
            self.set_status("not opening file (non-interactive)");
            return;
        }
        match spawn_system_open(path.as_os_str()) {
            Ok(_) => self.set_status("opened file externally"),
            Err(e) => {
                self.telemetry
                    .record(crate::telemetry::TelemetryEvent::ErrorOccurred {
                        module: "editor",
                        kind: "external_open_failed",
                    });
                self.set_status(format!("external open failed: {e}"));
            }
        }
    }
}

/// Spawns the OS-appropriate "open with default app" command for `arg`
/// (a URL or file path): `open` on macOS, `cmd /c start` on Windows, and
/// `xdg-open` elsewhere. Shared by `open_in_browser` and `open_external`.
fn spawn_system_open(arg: &std::ffi::OsStr) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(arg).spawn()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", ""])
            .arg(arg)
            .spawn()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open").arg(arg).spawn()
    }
}
