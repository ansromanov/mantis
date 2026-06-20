//! Subprocess-based plugin system for `tv`.
//!
//! Plugins are standalone executables that communicate via newline-delimited
//! JSON on stdin/stdout. `tv` sends lifecycle and hook events; plugins respond
//! with action events. A reader thread per plugin drains stdout non-blockingly
//! over a channel so the event loop never blocks on plugin I/O. A writer thread
//! per plugin drains a send queue to stdin, so slow or unresponsive plugins
//! cannot block the event loop on writes either.
//!
//! Protocol (tv → plugin, one JSON object per line on stdin):
//!   {"event":"init"}
//!   {"event":"on_file_open","path":"/some/file"}
//!   {"event":"on_keypress","key":"ctrl+p"}
//!   {"event":"on_selection_change","path":"/some/file"}
//!   {"event":"on_quit"}
//!   {"event":"shutdown"}
//!
//! Protocol (plugin → tv, one JSON object per line on stdout):
//!   {"event":"action","action":"show_message","params":{"message":"hello"}}
//!   {"event":"action","action":"open_file","params":{"path":"/tmp/x"}}

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

/// Per-plugin entry in the `[plugins]` section of `tv.toml`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct PluginEntry {
    /// Path to the plugin executable. Relative paths are resolved relative to
    /// the platform config directory (see `default_plugin_dir`).
    pub path: PathBuf,
    /// When `false` the plugin is registered but not spawned at startup.
    pub enabled: bool,
}

impl Default for PluginEntry {
    fn default() -> Self {
        PluginEntry {
            path: PathBuf::new(),
            enabled: true,
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
}

/// Message received from a plugin (on its stdout).
#[derive(Deserialize)]
struct FromPlugin {
    #[allow(dead_code)]
    event: String,
    action: Option<String>,
    #[serde(default)]
    params: HashMap<String, String>,
}

/// A single running plugin subprocess with background reader and writer threads.
pub struct Plugin {
    name: String,
    child: Option<Child>,
    /// Sends serialised JSON lines to the plugin's stdin via the writer thread.
    write_tx: Option<Sender<String>>,
    action_rx: Option<std::sync::mpsc::Receiver<(String, HashMap<String, String>)>>,
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

    fn drain_actions(&mut self) -> Vec<(String, HashMap<String, String>)> {
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
        drop(self.write_tx.take());
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(2);
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
}

/// Manages discovery, lifecycle, and hook dispatch for all plugins.
pub struct PluginManager {
    entries: Vec<(String, PluginEntry)>,
    plugins: Vec<Plugin>,
    pending_actions: Vec<(String, String, HashMap<String, String>)>,
    spawn_errors: Vec<String>,
}

impl PluginManager {
    pub fn new(entries: Vec<(String, PluginEntry)>) -> Self {
        PluginManager {
            entries,
            plugins: Vec::new(),
            pending_actions: Vec::new(),
            spawn_errors: Vec::new(),
        }
    }

    /// Spawns all enabled plugins and sends them the `init` event.
    pub fn activate_all(&mut self) {
        let plugin_dir = default_plugin_dir();
        for (name, entry) in &self.entries {
            if !entry.enabled {
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
    pub fn take_actions(&mut self) -> Vec<(String, String, HashMap<String, String>)> {
        std::mem::take(&mut self.pending_actions)
    }

    /// Whether any plugins are currently active.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Returns every registered plugin as `(name, is_running)`, in registration order.
    pub fn plugin_entries(&self) -> Vec<(String, bool)> {
        self.entries
            .iter()
            .map(|(name, _)| {
                let running = self.plugins.iter().any(|p| p.name == *name);
                (name.clone(), running)
            })
            .collect()
    }

    /// Spawns a single registered plugin by name and sends it the `init` event.
    /// No-op if already running. Returns an error string on spawn failure.
    pub fn activate_one(&mut self, name: &str) -> Result<(), String> {
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
        });
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
        });
        plugin.close();
    }
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

/// List of (filename, script_content) for each plugin that ships with tv.
/// Installed to the plugin directory by `install_bundled_plugins()`.
const BUNDLED_PLUGINS: &[(&str, &str)] = &[
    ("git-diff.sh", include_str!("../plugins/git-diff.sh")),
    ("git-log.sh", include_str!("../plugins/git-log.sh")),
];

/// Copies every bundled plugin to the plugin directory if it doesn't already
/// exist there, so users can inspect, edit, or register them in `tv.toml`.
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
}

/// Process-wide mutex for tests that mutate `XDG_CONFIG_HOME` / `APPDATA`.
/// Shared via `crate::plugin::ENV_LOCK` so that `theme_test.rs` and
/// `plugin_test.rs` serialise against each other (separate per-module statics
/// would not prevent concurrent mutations of the same env var).
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "plugin_test.rs"]
mod plugin_test;
