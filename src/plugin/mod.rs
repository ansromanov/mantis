//! Plugin system for `mantis`.
//!
//! Two kinds of plugins exist:
//!
//! 1. **Process plugins** — standalone executables that communicate via
//!    newline-delimited JSON on stdin/stdout. `mantis` sends lifecycle and hook
//!    events; plugins respond with action events. A reader thread per plugin
//!    drains stdout non-blockingly over a channel so the event loop never
//!    blocks on plugin I/O. A writer thread per plugin drains a send queue to
//!    stdin, so slow or unresponsive plugins cannot block the event loop on
//!    writes either.
//!
//!    Protocol (mantis → plugin, one JSON object per line on stdin):
//!
//!    ```json
//!    {"event":"init"}
//!    {"event":"on_file_open","path":"/some/file"}
//!    {"event":"on_keypress","key":"ctrl+p"}
//!    {"event":"on_selection_change","path":"/some/file"}
//!    {"event":"on_quit"}
//!    {"event":"shutdown"}
//!    {"event":"request","id":1,"method":"fold_regions","params":{"path":"/some/file"}}
//!    ```
//!
//!    Protocol (plugin → mantis, one JSON object per line on stdout):
//!
//!    ```json
//!    {"event":"action","action":"show_message","params":{"message":"hello"}}
//!    {"event":"action","action":"open_file","params":{"path":"/tmp/x"}}
//!    {"event":"action","action":"key_handled","params":{"handled":true}}
//!    {"event":"action","action":"plugin_error","params":{"message":"failed"}}
//!    {"event":"response","id":1,"result":{"regions":[[0,5]]}}
//!    ```
//!
//!    The `request`/`response` pair (protocol 3+) is additive to the
//!    event/action stream: a host `request` is answered by a correlated
//!    plugin `response` matched on `id`, with a per-plugin timeout if none
//!    arrives (see `crate::plugin::manager::PluginManager::send_request` and
//!    `poll_requests`).
//!
//! 2. **Syntax plugins** — provide a `.sublime-syntax` file that is loaded
//!    into the syntect highlighter at startup. No subprocess is spawned.
//!    Syntax plugins are declared in `mantis.toml` with `kind = "syntax"` and
//!    a `syntax_file` path.  Additionally, any `.sublime-syntax` file placed
//!    in `{plugin_dir}/syntaxes/` is auto-discovered.

/// Current plugin IPC protocol version.
///
/// Bumped on incompatible protocol changes. Plugins declare their protocol
/// version in `plugin.toml` via the `mantis_protocol` field (`tv_protocol` is
/// still accepted as a back-compat alias). Plugins whose declared version
/// does not match this constant are silently skipped during discovery to
/// prevent miscommunication.
///
/// History:
/// - `"1"` — initial protocol (0.7.x releases)
/// - `"2"` — language providers, event subscriptions, protocol hardening (0.8.x)
/// - `"3"` — request/response correlation (`request`/`response` events),
///   `plugin_error` action, `on_keypress` key consumption (`key_handled`),
///   `priority` on `register_language_provider`, manifest field renamed
///   `tv_protocol` -> `mantis_protocol` (alias kept) (0.14.x)
pub(crate) const PROTOCOL_VERSION: &str = "3";

pub mod install;
pub mod manifest;
pub mod registry;
pub mod types;

mod manager;
mod process;
mod syntax;

pub(crate) use install::bundled_plugin_entries;
pub(crate) use install::default_plugin_dir;
pub(crate) use install::install_bundled_plugins;
pub(crate) use install::retired_bundled_plugins;
pub(crate) use manager::PluginManager;
pub(crate) use syntax::load_extra_syntaxes;
pub(crate) use types::{
    Capability, ExtraSyntax, LanguageProviderRegistration, PluginContributions, PluginEntry,
    PluginKind,
};

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

/// Process-wide mutex for tests that mutate `XDG_CONFIG_HOME` / `APPDATA`.
/// Shared via `crate::plugin::ENV_LOCK` so that `theme_test.rs` and
/// `plugin_test.rs` serialise against each other (separate per-module statics
/// would not prevent concurrent mutations of the same env var).
#[cfg(test)]
pub(crate) static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
#[path = "mod_test.rs"]
mod plugin_test;

#[cfg(test)]
#[path = "install_test.rs"]
mod install_tests;

#[cfg(test)]
#[path = "manager_test.rs"]
mod manager_tests;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod manifest_tests;

#[cfg(test)]
#[path = "process_test.rs"]
mod process_tests;

#[cfg(test)]
#[path = "syntax_test.rs"]
mod syntax_tests;

#[cfg(test)]
#[path = "types_test.rs"]
mod types_tests;
