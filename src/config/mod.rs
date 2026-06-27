//! Loading, parsing, and saving of the `mantis.toml` configuration.
//!
//! Two layers: the **embedded defaults** (the fully-commented `mantis.toml` template
//! baked into the binary) supply every value, and the user's `mantis.toml` overrides
//! only the keys it sets — serde's `#[serde(default)]` merges the two. On launch
//! a read-only `mantis.default.toml` reference is (re)written next to the user config
//! whenever it is missing or stale, so an upgrade always refreshes the documented
//! option catalogue without ever touching the user's own file. The user `mantis.toml`
//! is created once as a minimal stub and from then on is only written by `save`,
//! which emits a *sparse* override file (changed-from-default keys only).
//!
//! Defines the `Config` struct (every user-tunable option, with serde defaults
//! so partial configs and older files still load) and the `Keymap`/keybinding
//! types that map config strings to `crossterm` key events. `load` locates and
//! deserializes the config, returning any validation warning rather than failing
//! the launch; `save` writes the current settings back. The `pressed` helper
//! tests a key event against a bound action's list. Unknown-key detection lives
//! in the `validate` submodule. Keep new fields here in sync with the defaults
//! so round-tripping a saved config is lossless.

mod validate;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(unix)]
use crate::event_source::CURRENT_BASE_KEY;

use crate::plugin::PluginEntry;
use crate::theme::ThemeConfig;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    pub tree_width: u16,
    pub tree_independent_scroll: bool,
    pub word_wrap: bool,
    pub line_numbers: bool,
    pub git_status: bool,
    pub git_show_deleted: bool,
    pub git_show_untracked: bool,
    pub git_show_ignored: bool,
    pub scrollbar: bool,
    pub scroll_percentage: bool,
    pub in_file_search: bool,
    pub search_context_lines: usize,
    pub keep_search_query: bool,
    /// Automatically reload file content when the open file changes on disk.
    /// Toggled at runtime with the `toggle_watch` keybinding.
    pub watch: bool,
    /// Maximum number of recently opened files to remember. Defaults to 10.
    pub recent_files_count: usize,
    /// Show file encoding and line-ending info in the status bar.
    pub show_file_info: bool,
    pub keys: Keymap,
    pub theme: ThemeConfig,
    /// Render indentation guide lines (│) in the tree pane.
    pub indent_guides: bool,
    /// Show Nerd Font file-type icons in the tree. The icon map is provided by
    /// a plugin (e.g. the bundled iconize plugin). Off by default — requires a
    /// Nerd Font in your terminal.
    pub icons: bool,
    /// Per-plugin entries registered in `[plugins]`.
    #[serde(default)]
    pub plugins: HashMap<String, PluginEntry>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            show_hidden: false,
            ignore_gitignore: false,
            tree_width: 28,
            tree_independent_scroll: false,
            word_wrap: false,
            line_numbers: true,
            git_status: true,
            git_show_deleted: false,
            git_show_untracked: true,
            git_show_ignored: false,
            scrollbar: true,
            scroll_percentage: true,
            in_file_search: true,
            search_context_lines: 0,
            keep_search_query: false,
            watch: false,
            recent_files_count: 10,
            show_file_info: true,
            keys: Keymap::default(),
            theme: ThemeConfig::default(),
            indent_guides: true,
            icons: false,
            plugins: HashMap::new(),
        }
    }
}

/// A single key combination, e.g. `q`, `ctrl+c`, `alt+.`, `cmd+p`, `PageUp`.
#[derive(Clone, Copy)]
pub struct KeyBinding {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
    super_key: bool,
}

impl KeyBinding {
    /// Whether a key event matches this binding. Shift is intentionally
    /// ignored because crossterm already encodes it in the char case.
    ///
    /// On terminals that support the kitty keyboard protocol with
    /// `REPORT_ALTERNATE_KEYS`, the event carries a *base key* — the
    /// US-layout physical key. When it is available, `matches` prefers the
    /// base key over `key.code` for `Char` bindings, so shortcuts work
    /// regardless of the active keyboard layout.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        #[cfg(unix)]
        let event_code = CURRENT_BASE_KEY
            .with(|cell| cell.get())
            .and_then(|bk| {
                // Only substitute for Char bindings; layout-dependent keys
                // (Enter, Space, Tab, arrows, …) are never affected.
                if matches!(self.code, KeyCode::Char(_)) {
                    Some(bk)
                } else {
                    None
                }
            })
            .unwrap_or(key.code);
        #[cfg(not(unix))]
        let event_code = key.code;

        event_code == self.code
            && key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt
            && key.modifiers.contains(KeyModifiers::SUPER) == self.super_key
    }

    /// Returns a human-readable label for this binding, e.g. `"Ctrl+P"`, `"Alt+."`.
    pub fn display(&self) -> String {
        let key = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            ref other => format!("{other:?}"),
        };
        match (self.ctrl, self.alt, self.super_key) {
            (true, true, false) => format!("Ctrl+Alt+{key}"),
            (true, false, false) => format!("Ctrl+{key}"),
            (false, true, false) => format!("Alt+{key}"),
            (false, false, true) => format!("Cmd+{key}"),
            (true, false, true) => format!("Ctrl+Cmd+{key}"),
            _ => key,
        }
    }
}

/// Returns true if any binding in the list matches the key event.
pub fn pressed(bindings: &[KeyBinding], key: &KeyEvent) -> bool {
    bindings.iter().any(|b| b.matches(key))
}

impl Keymap {
    /// Returns a display label for the first binding mapped to `action_id`,
    /// e.g. `"Ctrl+G"` for `toggle_git_mode`.
    pub fn label_for_action(&self, action_id: &str) -> String {
        let bindings: &[KeyBinding] = match action_id {
            "toggle_help" => &self.help,
            "toggle_hidden" => &self.toggle_hidden,
            "open_file_search" => &self.search_files,
            "open_content_search" => &self.search_content,
            "reload" => &self.reload,
            "open_file_history" => &self.file_history,
            "open_theme_picker" => &self.theme_picker,
            "toggle_git_mode" => &self.git_mode_toggle,
            "toggle_git_flat" => &self.git_mode_flat_toggle,
            "toggle_word_wrap" => &self.toggle_wrap,
            "toggle_line_numbers" => &self.toggle_line_numbers,
            "toggle_raw_markdown" => &self.toggle_raw_markdown,
            "toggle_pretty_json" => &self.toggle_pretty_json,
            "toggle_blame" => &self.toggle_blame,
            "toggle_diff_side_by_side" => &self.toggle_diff_side_by_side,
            "toggle_diff_staged" => &self.toggle_diff_staged,
            "open_in_editor" => &self.open_in_editor,
            "fold_toggle" => &self.fold_toggle,
            "toggle_watch" => &self.toggle_watch,
            "open_recent_files" => &self.recent_files,
            "copy_path" => &self.copy_path,
            "copy_relative_path" => &self.copy_relative_path,
            "open_plugin_picker" => &self.plugin_picker,
            "tree_collapse_all" => &self.tree_collapse_all,
            "tree_expand_all" => &self.tree_expand_all,
            "go_to_line" => &self.goto_line,
            "blame_line" => &self.blame_line,
            "tree_up_dir" => &self.tree_up_dir,
            _ => return String::new(),
        };
        bindings.first().map(|b| b.display()).unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Keymap {
    // Global
    pub quit: Vec<KeyBinding>,
    pub help: Vec<KeyBinding>,
    pub toggle_hidden: Vec<KeyBinding>,
    pub search_files: Vec<KeyBinding>,
    pub search_content: Vec<KeyBinding>,
    pub reload: Vec<KeyBinding>,
    pub switch_panel: Vec<KeyBinding>,
    pub file_history: Vec<KeyBinding>,
    pub theme_picker: Vec<KeyBinding>,
    // Shared navigation (tree + content)
    pub nav_up: Vec<KeyBinding>,
    pub nav_down: Vec<KeyBinding>,
    // Tree panel
    pub tree_expand: Vec<KeyBinding>,
    pub tree_collapse: Vec<KeyBinding>,
    pub tree_collapse_all: Vec<KeyBinding>,
    pub tree_expand_all: Vec<KeyBinding>,
    // Content panel
    pub content_left: Vec<KeyBinding>,
    pub content_right: Vec<KeyBinding>,
    pub content_top: Vec<KeyBinding>,
    pub content_bottom: Vec<KeyBinding>,
    pub content_page_up: Vec<KeyBinding>,
    pub content_page_down: Vec<KeyBinding>,
    pub content_reset_col: Vec<KeyBinding>,
    pub toggle_wrap: Vec<KeyBinding>,
    pub toggle_line_numbers: Vec<KeyBinding>,
    pub toggle_raw_markdown: Vec<KeyBinding>,
    pub toggle_pretty_json: Vec<KeyBinding>,
    pub toggle_blame: Vec<KeyBinding>,
    pub blame_line: Vec<KeyBinding>,
    pub toggle_diff_side_by_side: Vec<KeyBinding>,
    /// Cycles the active diff mode: all -> staged -> unstaged -> all.
    pub toggle_diff_staged: Vec<KeyBinding>,
    pub diff_hunk_next: Vec<KeyBinding>,
    pub diff_hunk_prev: Vec<KeyBinding>,
    pub git_mode_toggle: Vec<KeyBinding>,
    pub git_mode_flat_toggle: Vec<KeyBinding>,
    pub command_palette: Vec<KeyBinding>,
    pub open_in_editor: Vec<KeyBinding>,
    pub fold_toggle: Vec<KeyBinding>,
    pub toggle_watch: Vec<KeyBinding>,
    pub recent_files: Vec<KeyBinding>,
    pub copy_path: Vec<KeyBinding>,
    pub copy_relative_path: Vec<KeyBinding>,
    pub plugin_picker: Vec<KeyBinding>,
    pub goto_line: Vec<KeyBinding>,
    pub tree_up_dir: Vec<KeyBinding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap {
            quit: bind(&["q", "ctrl+c"]),
            help: bind(&["?"]),
            toggle_hidden: bind(&[".", "alt+."]),
            search_files: bind(&["/"]),
            search_content: bind(&["f"]),
            reload: bind(&["r"]),
            switch_panel: bind(&["Tab"]),
            file_history: bind(&["H"]),
            theme_picker: bind(&["t"]),
            nav_up: bind(&["Up", "k"]),
            nav_down: bind(&["Down", "j"]),
            tree_expand: bind(&["Enter", "Right", "l"]),
            tree_collapse: bind(&["Left", "h"]),
            tree_collapse_all: bind(&["-"]),
            tree_expand_all: bind(&["="]),
            content_left: bind(&["Left"]),
            content_right: bind(&["Right"]),
            content_top: bind(&["g", "Home"]),
            content_bottom: bind(&["G", "End"]),
            content_page_up: bind(&["PageUp"]),
            content_page_down: bind(&["PageDown"]),
            content_reset_col: bind(&["0"]),
            toggle_wrap: bind(&["z"]),
            toggle_line_numbers: bind(&["L"]),
            toggle_raw_markdown: bind(&["M"]),
            toggle_pretty_json: bind(&["J"]),
            toggle_blame: bind(&["b"]),
            blame_line: bind(&["B"]),
            toggle_diff_side_by_side: bind(&["D"]),
            toggle_diff_staged: bind(&["S"]),
            diff_hunk_next: bind(&["n"]),
            diff_hunk_prev: bind(&["N"]),
            git_mode_toggle: bind(&["ctrl+g"]),
            git_mode_flat_toggle: bind(&["F", "alt+g"]),
            command_palette: bind(&["ctrl+p"]),
            open_in_editor: bind(&["e"]),
            fold_toggle: bind(&["Space"]),
            toggle_watch: bind(&["W"]),
            recent_files: bind(&["ctrl+o"]),
            copy_path: bind(&["y"]),
            copy_relative_path: bind(&["Y"]),
            plugin_picker: bind(&["p"]),
            goto_line: bind(&[":"]),
            tree_up_dir: bind(&["Backspace"]),
        }
    }
}

/// Build a list of bindings from string specs. Panics on an invalid spec, so
/// it must only be used for the hardcoded defaults above.
fn bind(specs: &[&str]) -> Vec<KeyBinding> {
    specs
        .iter()
        .map(|s| parse_binding(s).expect("invalid default key binding"))
        .collect()
}

fn parse_binding(s: &str) -> Result<KeyBinding, String> {
    let parts: Vec<&str> = s.split('+').collect();
    let (mods, key) = parts.split_at(parts.len() - 1);

    let mut ctrl = false;
    let mut alt = false;
    let mut super_key = false;
    for m in mods {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "meta" => alt = true,
            "cmd" | "super" | "command" => super_key = true,
            "shift" => {} // encoded in the char case
            other => return Err(format!("unknown modifier '{other}' in '{s}'")),
        }
    }

    Ok(KeyBinding {
        code: parse_keycode(key[0], s)?,
        ctrl,
        alt,
        super_key,
    })
}

fn parse_keycode(s: &str, full: &str) -> Result<KeyCode, String> {
    // A single character is a literal key; preserve its case.
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.clone().next()) {
        return Ok(KeyCode::Char(c));
    }

    let code = match s.to_ascii_lowercase().as_str() {
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "enter" | "return" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "esc" | "escape" => KeyCode::Esc,
        "backspace" => KeyCode::Backspace,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "space" => KeyCode::Char(' '),
        _ => return Err(format!("unknown key '{s}' in '{full}'")),
    };
    Ok(code)
}

impl<'de> Deserialize<'de> for KeyBinding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        parse_binding(&s).map_err(serde::de::Error::custom)
    }
}

impl Serialize for KeyBinding {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let key = match self.code {
            KeyCode::Char(' ') => "space".to_string(),
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::PageUp => "PageUp".to_string(),
            KeyCode::PageDown => "PageDown".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            // Unreachable via parse_binding, but produce a non-failing placeholder
            // so save() always writes a complete config rather than silently dropping it.
            ref other => format!("{other:?}"),
        };
        let spec = match (self.ctrl, self.alt, self.super_key) {
            (true, true, false) => format!("ctrl+alt+{key}"),
            (true, false, false) => format!("ctrl+{key}"),
            (false, true, false) => format!("alt+{key}"),
            (false, false, true) => format!("cmd+{key}"),
            (true, false, true) => format!("ctrl+cmd+{key}"),
            _ => key,
        };
        s.serialize_str(&spec)
    }
}

/// Loads config for the given view root. A project-local `mantis.toml` found in
/// the root or any ancestor takes precedence over the global config; this lets
/// a repo ship its own defaults. On first run it seeds a minimal user config and
/// the bundled themes/plugins, and on every run refreshes the `mantis.default.toml`
/// reference; it never overwrites an existing user config. Returns the loaded
/// config, the path it was loaded from
/// (so that live changes are saved back to the same file), and a warning
/// describing the first malformed config encountered, if any, so the caller can
/// tell the user their config was ignored instead of failing silently.
pub fn load(root: &Path) -> (Config, Option<PathBuf>, Option<String>) {
    migrate_legacy_config();
    let global = global_config_path();
    if let Some(ref path) = global {
        init_config_dir(path);
    }
    let mut error = None;
    for path in config_paths(root) {
        let Ok(s) = fs::read_to_string(&path) else {
            continue; // missing or unreadable: try the next candidate
        };
        match toml::from_str::<Config>(&s) {
            Ok(config) => {
                // The config parsed, but `#[serde(default)]` silently ignores
                // unknown keys. Flag them (with nearest-match hints) so typos
                // don't get dropped without a word. A higher-precedence parse
                // failure already recorded above takes priority.
                if error.is_none() {
                    let unknown = validate::validate_keys(&s);
                    if !unknown.is_empty() {
                        error = Some(format!("{}: {}", path.display(), unknown.join("; ")));
                    }
                }
                return (config, Some(path), error);
            }
            // Record the first malformed config but keep falling back so a valid
            // lower-precedence file (e.g. the global config) can still load.
            Err(e) if error.is_none() => {
                error = Some(format!("{}: {e}", path.display()));
            }
            Err(_) => {}
        }
    }
    (Config::default(), global, error)
}

/// Writes `config` back to the user's `path` as a *sparse* override file: only
/// the keys whose value differs from the built-in defaults are written, so the
/// user config stays small and readable instead of growing into a full dump of
/// every setting. Silently ignores I/O errors.
pub fn save(config: &Config, path: &Path) {
    let _ = fs::write(path, sparse_toml(config));
}

/// Serialises `config` keeping only the top-level keys whose value differs from
/// `Config::default()`. This keeps the user's `mantis.toml` a minimal override file:
/// untouched settings fall through to the embedded defaults rather than being
/// pinned to their current value (which would also mask future default changes).
pub fn sparse_toml(config: &Config) -> String {
    let current = toml::Value::try_from(config);
    let default = toml::Value::try_from(Config::default());
    let (Ok(toml::Value::Table(cur)), Ok(toml::Value::Table(def))) = (current, default) else {
        // Serialisation should never fail for our own type; fall back to a full
        // dump rather than losing the user's settings.
        return toml::to_string_pretty(config).unwrap_or_default();
    };
    let mut out = toml::map::Map::new();
    for (k, v) in &cur {
        if def.get(k) != Some(v) {
            out.insert(k.clone(), v.clone());
        }
    }
    toml::to_string_pretty(&toml::Value::Table(out)).unwrap_or_default()
}

/// Prepares the global config directory on launch. The fully-commented default
/// reference (`mantis.default.toml`) is refreshed whenever it is missing or stale
/// (i.e. after an upgrade), so users always have an up-to-date catalogue of every
/// option. The user's own `mantis.toml` is **never** overwritten: it is created once,
/// as a minimal stub, only when absent. Bundled themes and plugins are seeded on
/// that same first run.
fn init_config_dir(user_path: &Path) {
    let Some(dir) = user_path.parent() else {
        return;
    };
    let _ = fs::create_dir_all(dir);
    refresh_default_reference(dir);
    if !user_path.exists() {
        let _ = fs::write(user_path, USER_CONFIG_STUB);
        crate::theme::install_embedded_themes();
        crate::plugin::install_bundled_plugins();
    }
}

/// Writes the embedded fully-commented template to `{dir}/mantis.default.toml`, but
/// only when the file is missing or its contents differ from the embedded
/// version (the upgrade case). Returns whether the file was (re)written. This is
/// a read-only reference for users; `mantis` itself reads values from the embedded
/// defaults, never from this file.
fn refresh_default_reference(dir: &Path) -> bool {
    let path = dir.join(DEFAULT_REFERENCE_NAME);
    if fs::read_to_string(&path).ok().as_deref() == Some(DEFAULT_CONFIG_TEMPLATE) {
        return false;
    }
    fs::write(&path, DEFAULT_CONFIG_TEMPLATE).is_ok()
}

/// The embedded, fully-commented default configuration. Source of truth for both
/// default values and the on-disk `mantis.default.toml` reference.
const DEFAULT_CONFIG_TEMPLATE: &str = include_str!("../../mantis.toml");

/// Filename of the read-only default reference written next to the user config.
const DEFAULT_REFERENCE_NAME: &str = "mantis.default.toml";

/// Minimal first-run user config. Kept deliberately tiny: the user adds only the
/// overrides they want, and consults `mantis.default.toml` for the full option list.
const USER_CONFIG_STUB: &str = "\
# mantis user config -- your overrides only.
#
# This file is never modified by upgrades. Add only the settings you want to
# change; everything else falls back to the built-in defaults.
#
# See mantis.default.toml in this directory (refreshed on every upgrade) for the
# full, commented list of available options.
";

/// Candidate config paths in precedence order: project-local (`mantis.toml` in the
/// root and each ancestor), then the global config.
fn config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = root.ancestors().map(|d| d.join("mantis.toml")).collect();
    if let Some(global) = global_config_path() {
        paths.push(global);
    }
    paths
}

fn global_config_path() -> Option<PathBuf> {
    dirs_next()?.join("mantis.toml").into()
}

fn dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("mantis"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("mantis"))
    }
}

/// One-time migration from the legacy `tree-viewer` config directory to `mantis`.
/// Called once at startup before any config resolution. If the new directory
/// (`~/.config/mantis/`) does not exist but the old (`~/.config/tree-viewer/`)
/// does, the old directory is renamed to the new name. Inside it, `tv.toml` is
/// renamed to `mantis.toml` and `tv.default.toml` to `mantis.default.toml`.
/// Best-effort: never destroys data on failure.
fn migrate_legacy_config() {
    let old_dir = legacy_dirs_next();
    let new_dir = dirs_next();
    let (Some(old), Some(new)) = (old_dir, new_dir) else {
        return;
    };
    if new.exists() || !old.exists() {
        return;
    }
    // Rename config files inside the old directory before moving the dir.
    for (old_name, new_name) in [
        ("tv.toml", "mantis.toml"),
        ("tv.default.toml", "mantis.default.toml"),
    ] {
        let old_file = old.join(old_name);
        let new_file = old.join(new_name);
        if old_file.exists() {
            let _ = fs::rename(&old_file, &new_file);
        }
    }
    // Rename the entire directory.
    let _ = fs::rename(&old, &new);
}

/// Returns the legacy config directory path (`tree-viewer`). Used only for
/// one-time migration.
fn legacy_dirs_next() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("tree-viewer"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|base| base.join("tree-viewer"))
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
