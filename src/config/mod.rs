use crate::theme::ThemeConfig;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    pub tree_width: u16,
    pub word_wrap: bool,
    pub git_status: bool,
    pub git_show_deleted: bool,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    pub scrollbar: bool,
    pub scroll_percentage: bool,
    pub in_file_search: bool,
    pub keys: Keymap,
    pub theme: ThemeConfig,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            show_hidden: false,
            ignore_gitignore: false,
            tree_width: 28,
            word_wrap: false,
            git_status: true,
            git_show_deleted: false,
            git_mode: false,
            git_mode_flat: false,
            scrollbar: true,
            scroll_percentage: true,
            in_file_search: true,
            keys: Keymap::default(),
            theme: ThemeConfig::default(),
        }
    }
}

/// A single key combination, e.g. `q`, `ctrl+c`, `alt+.`, `PageUp`.
#[derive(Clone, Copy)]
pub struct KeyBinding {
    code: KeyCode,
    ctrl: bool,
    alt: bool,
}

impl KeyBinding {
    /// Whether a key event matches this binding. Shift is intentionally
    /// ignored because crossterm already encodes it in the char case.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        key.code == self.code
            && key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt
    }
}

/// Returns true if any binding in the list matches the key event.
pub fn pressed(bindings: &[KeyBinding], key: &KeyEvent) -> bool {
    bindings.iter().any(|b| b.matches(key))
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
    // Content panel
    pub content_left: Vec<KeyBinding>,
    pub content_right: Vec<KeyBinding>,
    pub content_top: Vec<KeyBinding>,
    pub content_bottom: Vec<KeyBinding>,
    pub content_page_up: Vec<KeyBinding>,
    pub content_page_down: Vec<KeyBinding>,
    pub content_reset_col: Vec<KeyBinding>,
    pub toggle_wrap: Vec<KeyBinding>,
    pub toggle_raw_markdown: Vec<KeyBinding>,
    pub git_mode_toggle: Vec<KeyBinding>,
    pub git_mode_flat_toggle: Vec<KeyBinding>,
}

impl Default for Keymap {
    fn default() -> Self {
        Keymap {
            quit: bind(&["q", "ctrl+c"]),
            help: bind(&["?"]),
            toggle_hidden: bind(&["alt+."]),
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
            content_left: bind(&["Left"]),
            content_right: bind(&["Right"]),
            content_top: bind(&["g"]),
            content_bottom: bind(&["G"]),
            content_page_up: bind(&["PageUp"]),
            content_page_down: bind(&["PageDown"]),
            content_reset_col: bind(&["0"]),
            toggle_wrap: bind(&["z"]),
            toggle_raw_markdown: bind(&["M"]),
            git_mode_toggle: bind(&["ctrl+g"]),
            git_mode_flat_toggle: bind(&["alt+g"]),
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
    for m in mods {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" | "meta" | "option" => alt = true,
            "shift" => {} // encoded in the char case
            other => return Err(format!("unknown modifier '{other}' in '{s}'")),
        }
    }

    Ok(KeyBinding {
        code: parse_keycode(key[0], s)?,
        ctrl,
        alt,
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
        let spec = match (self.ctrl, self.alt) {
            (true, true) => format!("ctrl+alt+{key}"),
            (true, false) => format!("ctrl+{key}"),
            (false, true) => format!("alt+{key}"),
            (false, false) => key,
        };
        s.serialize_str(&spec)
    }
}

/// Loads config for the given view root. A project-local `tv.toml` found in
/// the root or any ancestor takes precedence over the global config; this lets
/// a repo ship its own defaults. Creates the global config with defaults if it
/// doesn't exist yet. Returns the loaded config, the path it was loaded from
/// (so that live changes are saved back to the same file), and a warning
/// describing the first malformed config encountered, if any, so the caller can
/// tell the user their config was ignored instead of failing silently.
pub fn load(root: &Path) -> (Config, Option<PathBuf>, Option<String>) {
    let global = global_config_path();
    if let Some(ref path) = global {
        if !path.exists() {
            install_default(path);
        }
    }
    let mut error = None;
    for path in config_paths(root) {
        let Ok(s) = fs::read_to_string(&path) else {
            continue; // missing or unreadable: try the next candidate
        };
        match toml::from_str::<Config>(&s) {
            Ok(config) => return (config, Some(path), error),
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

/// Writes `config` to `path`, silently ignoring errors.
pub fn save(config: &Config, path: &Path) {
    if let Ok(content) = toml::to_string_pretty(config) {
        let _ = fs::write(path, content);
    }
}

/// Creates the global config file with defaults if the directory is writable.
fn install_default(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    save(&Config::default(), path);
}

/// Candidate config paths in precedence order: project-local (`tv.toml` in the
/// root and each ancestor), then the global config.
fn config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = root.ancestors().map(|d| d.join("tv.toml")).collect();
    if let Some(global) = global_config_path() {
        paths.push(global);
    }
    paths
}

fn global_config_path() -> Option<PathBuf> {
    dirs_next()?.join("tv.toml").into()
}

fn dirs_next() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

#[cfg(test)]
mod tests;
