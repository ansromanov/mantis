//! Plugin system for `tv`.
//!
//! Two kinds of plugins exist:
//!
//! 1. **Process plugins** — standalone executables that communicate via
//!    newline-delimited JSON on stdin/stdout. `tv` sends lifecycle and hook
//!    events; plugins respond with action events. A reader thread per plugin
//!    drains stdout non-blockingly over a channel so the event loop never
//!    blocks on plugin I/O. A writer thread per plugin drains a send queue to
//!    stdin, so slow or unresponsive plugins cannot block the event loop on
//!    writes either.
//!
//!    Protocol (tv → plugin, one JSON object per line on stdin):
//!
//!    ```json
//!    {"event":"init"}
//!    {"event":"on_file_open","path":"/some/file"}
//!    {"event":"on_keypress","key":"ctrl+p"}
//!    {"event":"on_selection_change","path":"/some/file"}
//!    {"event":"on_quit"}
//!    {"event":"shutdown"}
//!    ```
//!
//!    Protocol (plugin → tv, one JSON object per line on stdout):
//!
//!    ```json
//!    {"event":"action","action":"show_message","params":{"message":"hello"}}
//!    {"event":"action","action":"open_file","params":{"path":"/tmp/x"}}
//!    ```
//!
//! 2. **Syntax plugins** — provide a `.sublime-syntax` file that is loaded
//!    into the syntect highlighter at startup. No subprocess is spawned.
//!    Syntax plugins are declared in `tv.toml` with `kind = "syntax"` and
//!    a `syntax_file` path.  Additionally, any `.sublime-syntax` file placed
//!    in `{plugin_dir}/syntaxes/` is auto-discovered.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// What kind of plugin this is.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    /// Standard subprocess plugin (the default).
    #[default]
    Process,
    /// A syntax-definition plugin: provides a `.sublime-syntax` file to extend
    /// the highlighter. No subprocess is spawned.
    Syntax,
}

/// A syntax definition loaded from a plugin, ready to be fed to syntect.
#[derive(Clone, Debug)]
pub struct ExtraSyntax {
    /// Path to the `.sublime-syntax` file on disk.
    pub syntax_path: PathBuf,
    /// File extensions this syntax should match (e.g. `["tf", "tfvars"]`).
    /// May be empty when the syntax definition declares them internally.
    /// Currently unused by the highlighter (extensions come from the syntax
    /// definition itself); reserved for future explicit mapping.
    #[allow(dead_code)]
    pub extensions: Vec<String>,
}

/// Per-plugin entry in the `[plugins]` section of `tv.toml`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PluginEntry {
    /// Path to the plugin executable (process plugins) or syntax file
    /// (syntax plugins). Relative paths are resolved relative to the platform
    /// config directory (see `default_plugin_dir`).
    pub path: PathBuf,
    /// When `false` the plugin is registered but not spawned at startup.
    pub enabled: bool,
    /// Plugin kind. Defaults to `"process"` for backward compatibility.
    pub kind: PluginKind,
    /// File extensions this syntax plugin handles (e.g. `["tf", "tfvars"]`).
    /// Only meaningful when `kind = "syntax"`.
    #[serde(default)]
    pub extensions: Vec<String>,
    /// Path to the `.sublime-syntax` file. Only meaningful when
    /// `kind = "syntax"`. Relative paths are resolved against the plugin dir.
    #[serde(default)]
    pub syntax_file: Option<PathBuf>,
}

impl Default for PluginEntry {
    fn default() -> Self {
        PluginEntry {
            path: PathBuf::new(),
            enabled: true,
            kind: PluginKind::Process,
            extensions: Vec::new(),
            syntax_file: None,
        }
    }
}

/// Message sent from `tv` to a plugin (on its stdin).
#[derive(Serialize)]
struct ToPlugin {
    event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    theme: Option<String>,
}

/// Message received from a plugin (on its stdout).
#[derive(Deserialize)]
struct FromPlugin {
    #[allow(dead_code)]
    event: String,
    action: Option<String>,
    #[serde(default)]
    params: serde_json::Value,
}

/// A single running plugin subprocess with background reader and writer threads.
pub struct Plugin {
    name: String,
    child: Option<Child>,
    /// Sends serialised JSON lines to the plugin's stdin via the writer thread.
    write_tx: Option<Sender<String>>,
    action_rx: Option<std::sync::mpsc::Receiver<(String, serde_json::Value)>>,
    _reader_thread: Option<std::thread::JoinHandle<()>>,
    _writer_thread: Option<std::thread::JoinHandle<()>>,
}

impl Plugin {
    fn new(name: String) -> Self {
        Plugin {
            name,
            child: None,
            write_tx: None,
            action_rx: None,
            _reader_thread: None,
            _writer_thread: None,
        }
    }

    fn spawn(&mut self, path: &Path) -> Result<(), String> {
        let mut child = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn plugin '{}': {}", self.name, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("no stdin for plugin '{}'", self.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("no stdout for plugin '{}'", self.name))?;

        let (read_tx, read_rx) = std::sync::mpsc::sync_channel(1024);
        let read_handle = std::thread::Builder::new()
            .name(format!("plugin-reader-{}", self.name))
            .spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(line) => {
                            let trimmed = line.trim();
                            if trimmed.is_empty() {
                                continue;
                            }
                            if let Ok(msg) = serde_json::from_str::<FromPlugin>(trimmed) {
                                if msg.event == "action" {
                                    if let Some(action) = msg.action {
                                        let _ = read_tx.try_send((action, msg.params));
                                    }
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn reader thread: {e}"))?;

        let (write_tx, write_rx) = std::sync::mpsc::channel::<String>();
        let write_handle = std::thread::Builder::new()
            .name(format!("plugin-writer-{}", self.name))
            .spawn(move || {
                let mut stdin = stdin;
                for msg in write_rx {
                    if writeln!(stdin, "{msg}").is_err() {
                        break;
                    }
                    let _ = stdin.flush();
                }
            })
            .map_err(|e| format!("failed to spawn writer thread: {e}"))?;

        self.child = Some(child);
        self.write_tx = Some(write_tx);
        self.action_rx = Some(read_rx);
        self._reader_thread = Some(read_handle);
        self._writer_thread = Some(write_handle);
        Ok(())
    }

    /// Enqueues a message for the writer thread; never blocks the caller.
    fn send(&mut self, msg: &ToPlugin) {
        let Some(ref write_tx) = self.write_tx else {
            return;
        };
        let Ok(json) = serde_json::to_string(msg) else {
            return;
        };
        let _ = write_tx.send(json);
    }

    fn drain_actions(&mut self) -> Vec<(String, serde_json::Value)> {
        let mut actions = Vec::new();
        let Some(ref rx) = self.action_rx else {
            return actions;
        };
        while let Ok((action, params)) = rx.try_recv() {
            actions.push((action, params));
        }
        actions
    }

    /// Drops the write channel (so the writer thread flushes and exits), then
    /// waits up to 2 s for the child to exit before killing it.
    ///
    /// Callers must have already sent `shutdown` via `send()` before calling
    /// this (e.g. `deactivate_all` does so).
    fn close(&mut self) {
        self.close_with_timeout(Duration::from_secs(2));
    }

    fn close_with_timeout(&mut self, timeout: Duration) {
        drop(self.write_tx.take());
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + timeout;
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() >= deadline => {
                        let _ = child.kill();
                        let _ = child.wait();
                        break;
                    }
                    _ => std::thread::sleep(Duration::from_millis(50)),
                }
            }
        }
    }

    /// Non-blocking shutdown: drops stdin (signals the plugin to exit), then
    /// moves the child process into a background thread that waits up to 2 s
    /// for a clean exit before force-killing it. The reader/writer thread
    /// handles are dropped here and exit naturally as their channels close.
    ///
    /// If the background thread cannot be spawned (resource exhaustion), the
    /// child is reaped synchronously on the current thread to avoid zombies.
    fn close_in_background(mut self) {
        drop(self.write_tx.take());
        if let Some(child) = self.child.take() {
            let name = self.name.clone();
            let child = std::sync::Arc::new(std::sync::Mutex::new(child));
            let bg = child.clone();
            if std::thread::Builder::new()
                .name(format!("plugin-closer-{name}"))
                .spawn(move || {
                    let mut c = bg.lock().unwrap_or_else(|e| e.into_inner());
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        match c.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) if Instant::now() >= deadline => {
                                let _ = c.kill();
                                let _ = c.wait();
                                break;
                            }
                            _ => std::thread::sleep(Duration::from_millis(50)),
                        }
                    }
                })
                .is_err()
            {
                if let Ok(mut c) = child.lock() {
                    let _ = c.kill();
                    let _ = c.wait();
                }
            }
        }
    }
}

/// Manages discovery, lifecycle, and hook dispatch for all plugins.
pub struct PluginManager {
    entries: Vec<(String, PluginEntry)>,
    plugins: Vec<Plugin>,
    pending_actions: Vec<(String, String, serde_json::Value)>,
    spawn_errors: Vec<String>,
    active_theme: Option<String>,
}

impl PluginManager {
    pub fn new(entries: Vec<(String, PluginEntry)>) -> Self {
        PluginManager {
            entries,
            plugins: Vec::new(),
            pending_actions: Vec::new(),
            spawn_errors: Vec::new(),
            active_theme: None,
        }
    }

    /// Spawns all enabled *process* plugins and sends them the `init` event.
    /// Syntax plugins are not spawned — they are consumed by the highlighter.
    /// The `theme_name` is sent in the `init` payload so the plugin is aware
    /// of the active theme from the start.
    pub fn activate_all(&mut self, theme_name: Option<&str>) {
        self.active_theme = theme_name.map(|s| s.to_string());
        let plugin_dir = default_plugin_dir();
        for (name, entry) in &self.entries {
            if !entry.enabled || entry.kind != PluginKind::Process {
                continue;
            }
            let path = if entry.path.is_relative() {
                plugin_dir.join(&entry.path)
            } else {
                entry.path.clone()
            };
            let mut plugin = Plugin::new(name.clone());
            if let Err(e) = plugin.spawn(&path) {
                self.spawn_errors.push(e);
                continue;
            }
            plugin.send(&ToPlugin {
                event: "init".into(),
                path: None,
                key: None,
                theme: self.active_theme.clone(),
            });
            self.plugins.push(plugin);
        }
    }

    /// Returns (and clears) any errors that occurred while spawning plugins
    /// during `activate_all`. Intended to be called once after `activate_all`.
    pub fn take_spawn_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.spawn_errors)
    }

    /// Sends `shutdown` to all plugins, then closes each subprocess (with a
    /// per-plugin 2-second timeout before forceful kill).
    pub fn deactivate_all(&mut self) {
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "shutdown".into(),
                path: None,
                key: None,
                theme: None,
            });
        }
        for mut plugin in self.plugins.drain(..) {
            plugin.close();
        }
    }

    /// Sends `on_file_open` to all active plugins.
    pub fn on_file_open(&mut self, path: &Path) {
        let path_s = path.to_string_lossy().into_owned();
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "on_file_open".into(),
                path: Some(path_s.clone()),
                key: None,
                theme: None,
            });
        }
    }

    /// Sends `on_keypress` to all active plugins with a human-readable key
    /// representation (e.g. `"q"`, `"ctrl+c"`, `"Enter"`).
    pub fn on_keypress(&mut self, key: &crossterm::event::KeyEvent) {
        let key_str = key_event_to_string(key);
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "on_keypress".into(),
                path: None,
                key: Some(key_str.clone()),
                theme: None,
            });
        }
    }

    /// Sends `on_theme_change` to all active plugins with the new theme name.
    pub fn on_theme_change(&mut self, theme: &str) {
        self.active_theme = Some(theme.to_string());
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "on_theme_change".into(),
                path: None,
                key: None,
                theme: Some(theme.into()),
            });
        }
    }

    /// Sends `on_selection_change` to all active plugins.
    pub fn on_selection_change(&mut self, path: Option<&Path>) {
        let path_s = path.map(|p| p.to_string_lossy().into_owned());
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: path_s.clone(),
                key: None,
                theme: None,
            });
        }
    }

    /// Sends `on_quit` to all active plugins (graceful shutdown notice).
    pub fn on_quit(&mut self) {
        for plugin in &mut self.plugins {
            plugin.send(&ToPlugin {
                event: "on_quit".into(),
                path: None,
                key: None,
                theme: None,
            });
        }
    }

    /// Non-blockingly drains pending actions from every plugin's reader channel
    /// into an internal buffer. Call `take_actions` to collect them.
    pub fn drain_actions(&mut self) {
        for plugin in &mut self.plugins {
            for (action, params) in plugin.drain_actions() {
                self.pending_actions
                    .push((plugin.name.clone(), action, params));
            }
        }
    }

    /// Consumes and returns all buffered plugin actions since the last call:
    /// `Vec<(plugin_name, action, params)>`.
    pub fn take_actions(&mut self) -> Vec<(String, String, serde_json::Value)> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Whether any plugins are currently active.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns every registered plugin as `(name, is_running)`, in the order held
    /// by this manager (set at construction time; `App::new` sorts by name).
    pub fn plugin_entries(&self) -> Vec<(String, bool)> {
        self.entries
            .iter()
            .map(|(name, _)| {
                let running = self.plugins.iter().any(|p| p.name == *name);
                (name.clone(), running)
            })
            .collect()
    }

    /// Spawns a single registered plugin by name, sends it `init`, and
    /// optionally follows up with `on_file_open` + `on_selection_change` for
    /// `current_file` so the plugin has the current UI state immediately.
    /// No-op if already running. Returns an error string on spawn failure.
    pub fn activate_one(&mut self, name: &str, current_file: Option<&Path>) -> Result<(), String> {
        if self.plugins.iter().any(|p| p.name == name) {
            return Ok(());
        }
        let entry = self
            .entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, e)| e.clone())
            .ok_or_else(|| format!("plugin '{name}' not registered"))?;
        let plugin_dir = default_plugin_dir();
        let path = if entry.path.is_relative() {
            plugin_dir.join(&entry.path)
        } else {
            entry.path.clone()
        };
        let mut plugin = Plugin::new(name.to_string());
        plugin.spawn(&path)?;
        plugin.send(&ToPlugin {
            event: "init".into(),
            path: None,
            key: None,
            theme: self.active_theme.clone(),
        });
        if let Some(file) = current_file {
            let path_s = file.to_string_lossy().into_owned();
            plugin.send(&ToPlugin {
                event: "on_file_open".into(),
                path: Some(path_s.clone()),
                key: None,
                theme: None,
            });
            plugin.send(&ToPlugin {
                event: "on_selection_change".into(),
                path: Some(path_s),
                key: None,
                theme: None,
            });
        }
        self.plugins.push(plugin);
        Ok(())
    }

    /// Sends `shutdown` to a single running plugin and closes its subprocess.
    /// No-op if no plugin with that name is running.
    pub fn deactivate_one(&mut self, name: &str) {
        let Some(pos) = self.plugins.iter().position(|p| p.name == name) else {
            return;
        };
        let mut plugin = self.plugins.remove(pos);
        plugin.send(&ToPlugin {
            event: "shutdown".into(),
            path: None,
            key: None,
            theme: None,
        });
        plugin.close_in_background();
    }
}

/// Collects `ExtraSyntax` entries from `[plugins]` entries whose
/// `kind = "syntax"`. The `syntax_file` path is resolved against the default
/// plugin directory when relative.
pub fn collect_syntax_plugins(entries: &[(String, PluginEntry)]) -> Vec<ExtraSyntax> {
    let plugin_dir = default_plugin_dir();
    entries
        .iter()
        .filter(|(_, e)| e.kind == PluginKind::Syntax && e.enabled)
        .filter_map(|(_, entry)| {
            let syntax_path = entry.syntax_file.as_ref()?;
            let path = if syntax_path.is_relative() {
                plugin_dir.join(syntax_path)
            } else {
                syntax_path.clone()
            };
            Some(ExtraSyntax {
                syntax_path: path,
                extensions: entry.extensions.clone(),
            })
        })
        .collect()
}

/// Auto-discovers `.sublime-syntax` files in `{plugin_dir}/syntaxes/`.
/// These are loaded regardless of whether an explicit `[plugins]` entry exists.
pub fn discover_syntax_plugins() -> Vec<ExtraSyntax> {
    let syntax_dir = default_plugin_dir().join("syntaxes");
    if !syntax_dir.is_dir() {
        return Vec::new();
    }
    let mut extra = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&syntax_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "sublime-syntax") {
                extra.push(ExtraSyntax {
                    syntax_path: path,
                    extensions: Vec::new(),
                });
            }
        }
    }
    extra
}

/// Combines config-based and auto-discovered syntax plugins into a single
/// list of extra syntax definitions for the highlighter. Deduplicates by
/// path (so an explicit `[plugins]` entry for a file that is also
/// auto-discovered does not load it twice) and sorts for determinism.
pub fn load_extra_syntaxes(entries: &[(String, PluginEntry)]) -> Vec<ExtraSyntax> {
    let mut extra = collect_syntax_plugins(entries);
    extra.extend(discover_syntax_plugins());
    let mut seen = std::collections::HashSet::new();
    extra.retain(|e| seen.insert(e.syntax_path.clone()));
    extra.sort_by(|a, b| a.syntax_path.cmp(&b.syntax_path));
    extra
}

/// Converts a crossterm `KeyEvent` into a human-readable string like `"q"`,
/// `"ctrl+c"`, `"Enter"`, `"alt+."`.
pub(crate) fn key_event_to_string(key: &crossterm::event::KeyEvent) -> String {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut parts = Vec::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("ctrl");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        parts.push("alt");
    }
    if key.modifiers.contains(KeyModifiers::SUPER) {
        parts.push("super");
    }
    let key_name = match key.code {
        KeyCode::Char(' ') => "Space".into(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Enter => "Enter".into(),
        KeyCode::Tab => "Tab".into(),
        KeyCode::Esc => "Esc".into(),
        KeyCode::Backspace => "Backspace".into(),
        KeyCode::Up => "Up".into(),
        KeyCode::Down => "Down".into(),
        KeyCode::Left => "Left".into(),
        KeyCode::Right => "Right".into(),
        KeyCode::PageUp => "PageUp".into(),
        KeyCode::PageDown => "PageDown".into(),
        KeyCode::Home => "Home".into(),
        KeyCode::End => "End".into(),
        _ => format!("{:?}", key.code),
    };
    if parts.is_empty() {
        key_name
    } else {
        format!("{}+{}", parts.join("+"), key_name)
    }
}

/// Tries to find `cargo` on PATH. Returns `Some(path)` when found.
fn which_cargo() -> Option<String> {
    if let Ok(cargo) = std::env::var("CARGO") {
        return Some(cargo);
    }
    for dir in std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let cand = dir.join(if cfg!(windows) { "cargo.exe" } else { "cargo" });
        if cand.is_file() {
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

/// Default plugin discovery directory.
///
/// - Linux/macOS: `$XDG_CONFIG_HOME/tree-viewer/plugins/` (falls back to
///   `~/.config/tree-viewer/plugins/` when the variable is unset)
/// - Windows:     `%APPDATA%\tree-viewer\plugins\`
pub(crate) fn default_plugin_dir() -> PathBuf {
    dirs_next().unwrap_or_else(|| PathBuf::from("."))
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("tree-viewer").join("plugins"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("tree-viewer").join("plugins"))
    }
}

/// List of (filename, script_content) for each process plugin that ships with tv.
/// Installed to the plugin directory by `install_bundled_plugins()`.
const BUNDLED_PLUGINS: &[(&str, &str)] = &[
    ("git-diff.sh", include_str!("../plugins/git-diff.sh")),
    ("git-log.sh", include_str!("../plugins/git-log.sh")),
];

/// List of (filename, content) for each bundled syntax definition.
/// Installed to `{plugin_dir}/syntaxes/` by `install_bundled_plugins()`.
const BUNDLED_SYNTAX_PLUGINS: &[(&str, &str)] = &[(
    "terraform.sublime-syntax",
    include_str!("../plugins/terraform.sublime-syntax"),
)];

/// Copies every bundled plugin to the plugin directory if it doesn't already
/// exist there, so users can inspect, edit, or register them in `tv.toml`.
/// Process plugins go directly into `{plugin_dir}/`; syntax definitions go
/// into `{plugin_dir}/syntaxes/` (auto-discovered at startup).
pub fn install_bundled_plugins() {
    let dir = default_plugin_dir();
    let _ = std::fs::create_dir_all(&dir);
    for (name, script) in BUNDLED_PLUGINS {
        let path = dir.join(name);
        if !path.exists() {
            let _ = std::fs::write(&path, script);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    // Install the bundled markdown plugin (Rust binary).
    // Searches alongside the tv binary, then in ../target/debug/ and
    // ../target/release/ relative to the project root (development builds).
    let md_plugin_name = "tv-plugin-markdown";
    let md_plugin_path = dir.join(md_plugin_name);
    if !md_plugin_path.exists() {
        let binary_name = if cfg!(windows) {
            format!("{md_plugin_name}.exe")
        } else {
            md_plugin_name.to_string()
        };
        // Try: next to the tv binary, target/debug, target/release
        let candidates: Vec<PathBuf> = {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.to_path_buf()));
            let mut c = Vec::new();
            if let Some(ref d) = exe_dir {
                c.push(d.join(&binary_name));
                c.push(d.join("..").join("debug").join(&binary_name));
                c.push(d.join("..").join("release").join(&binary_name));
            }
            c.push(PathBuf::from("target/debug").join(&binary_name));
            c.push(PathBuf::from("target/release").join(&binary_name));
            c
        };
        let mut found = false;
        for cand in &candidates {
            if cand.exists() {
                if std::fs::copy(cand, &md_plugin_path).is_ok() {
                    set_executable(&md_plugin_path);
                    found = true;
                }
                break;
            }
        }
        if !found {
            // Last resort: attempt to build with cargo.
            if let Some(cargo) = which_cargo() {
                let project_manifest = PathBuf::from("Cargo.toml");
                if project_manifest.exists() {
                    let _ = Command::new(&cargo)
                        .arg("build")
                        .arg("--package")
                        .arg(md_plugin_name)
                        .arg("--release")
                        .status()
                        .ok();
                    let release_path = PathBuf::from("target/release").join(&binary_name);
                    if release_path.exists() {
                        let _ = std::fs::copy(&release_path, &md_plugin_path);
                        set_executable(&md_plugin_path);
                    }
                }
            }
        }
    }
    // Install syntax files to syntaxes/ subdirectory for auto-discovery.
    let syntax_dir = dir.join("syntaxes");
    let _ = std::fs::create_dir_all(&syntax_dir);
    for (name, content) in BUNDLED_SYNTAX_PLUGINS {
        let path = syntax_dir.join(name);
        if !path.exists() {
            let _ = std::fs::write(&path, content);
        }
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755));
}
#[cfg(not(unix))]
fn set_executable(_path: &Path) {}

/// Process-wide mutex for tests that mutate `XDG_CONFIG_HOME` / `APPDATA`.
/// Shared via `crate::plugin::ENV_LOCK` so that `theme_test.rs` and
/// `plugin_test.rs` serialise against each other (separate per-module statics
/// would not prevent concurrent mutations of the same env var).
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "plugin_test.rs"]
mod plugin_test;
