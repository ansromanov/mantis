//! Launching external programs: `$EDITOR`, the browser, the OS opener and the file manager.

use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};

use std::io::IsTerminal;
use std::path::Path;

use super::super::App;

impl App {
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

    /// Opens the currently selected file in the user's `$EDITOR`. Delegates
    /// to `launch_editor` for the TUI suspend/resume dance, then reloads the
    /// file content.
    pub(crate) fn open_in_editor(&mut self) {
        if let Some(p) = self.current_file.clone() {
            self.launch_editor(&p);
            self.reload_content();
        }
    }

    /// Opens the config file in the user's `$EDITOR`. Delegates to
    /// `launch_editor` for the TUI suspend/resume dance, then re-reads config.
    pub(super) fn open_config_in_editor(&mut self) {
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
    pub(crate) fn open_external(&mut self, path: &Path) {
        if !std::io::stdout().is_terminal() {
            self.set_status("not opening file (non-interactive)");
            return;
        }
        match spawn_system_open(path.as_os_str()) {
            Ok(_) => self.set_status("opened file externally"),
            Err(e) => {
                self.set_status(format!("external open failed: {e}"));
            }
        }
    }

    /// Reveals `path` in the OS file manager: directories open directly, files
    /// are revealed in their parent folder. Mirrors the non-interactive guard
    /// of [`App::open_external`].
    pub(crate) fn open_in_file_manager(&mut self, path: &Path) {
        if !std::io::stdout().is_terminal() {
            self.set_status("not opening file manager (non-interactive)");
            return;
        }
        match spawn_reveal_in_file_manager(path) {
            Ok(_) => self.set_status(format!("revealed {}", path.display())),
            Err(e) => {
                self.set_status(format!("file manager open failed: {e}"));
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

/// Spawns the OS file manager to reveal `path`: directories open directly,
/// files are revealed in their parent folder. macOS prefixes `open` with `-R`
/// to reveal the file, Windows uses `explorer /select,`, and other platforms
/// open the parent folder with `xdg-open`.
fn spawn_reveal_in_file_manager(path: &Path) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .args(["/select,"])
            .arg(path)
            .spawn()
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let target = path
            .parent()
            .filter(|_| !path.is_dir())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf());
        std::process::Command::new("xdg-open").arg(target).spawn()
    }
}
