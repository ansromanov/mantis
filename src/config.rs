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
/// doesn't exist yet. Returns the loaded config and the path it was loaded from
/// so that live changes are saved back to the same file.
pub fn load(root: &Path) -> (Config, Option<PathBuf>) {
    let global = global_config_path();
    if let Some(ref path) = global {
        if !path.exists() {
            install_default(path);
        }
    }
    for path in config_paths(root) {
        if let Ok(s) = fs::read_to_string(&path) {
            if let Ok(config) = toml::from_str::<Config>(&s) {
                return (config, Some(path));
            }
        }
    }
    (Config::default(), global)
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
mod tests {
    use super::*;

    fn ev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn parses_single_char_preserving_case() {
        let g = parse_binding("G").unwrap();
        assert_eq!(g.code, KeyCode::Char('G'));
        assert!(!g.ctrl && !g.alt);

        let lower = parse_binding("g").unwrap();
        assert_eq!(lower.code, KeyCode::Char('g'));
    }

    #[test]
    fn parses_named_keys_case_insensitively() {
        assert_eq!(parse_binding("Up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_binding("up").unwrap().code, KeyCode::Up);
        assert_eq!(parse_binding("PAGEUP").unwrap().code, KeyCode::PageUp);
        assert_eq!(parse_binding("enter").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_binding("return").unwrap().code, KeyCode::Enter);
        assert_eq!(parse_binding("esc").unwrap().code, KeyCode::Esc);
        assert_eq!(parse_binding("space").unwrap().code, KeyCode::Char(' '));
    }

    #[test]
    fn parses_modifiers() {
        let c = parse_binding("ctrl+c").unwrap();
        assert_eq!(c.code, KeyCode::Char('c'));
        assert!(c.ctrl && !c.alt);

        let dot = parse_binding("alt+.").unwrap();
        assert_eq!(dot.code, KeyCode::Char('.'));
        assert!(dot.alt && !dot.ctrl);

        let both = parse_binding("ctrl+alt+x").unwrap();
        assert!(both.ctrl && both.alt);
        assert_eq!(both.code, KeyCode::Char('x'));
    }

    #[test]
    fn modifier_aliases_accepted() {
        assert!(parse_binding("control+a").unwrap().ctrl);
        assert!(parse_binding("meta+a").unwrap().alt);
        assert!(parse_binding("option+a").unwrap().alt);
    }

    #[test]
    fn shift_modifier_is_ignored_in_spec() {
        // Shift is encoded in char case, so it is parsed but sets no flag.
        let b = parse_binding("shift+a").unwrap();
        assert!(!b.ctrl && !b.alt);
        assert_eq!(b.code, KeyCode::Char('a'));
    }

    #[test]
    fn rejects_unknown_modifier_and_key() {
        assert!(parse_binding("hyper+a").is_err());
        assert!(parse_binding("nope").is_err());
    }

    #[test]
    fn matches_requires_exact_modifiers() {
        let b = parse_binding("ctrl+c").unwrap();
        assert!(b.matches(&ev(KeyCode::Char('c'), KeyModifiers::CONTROL)));
        // Missing the ctrl modifier should not match.
        assert!(!b.matches(&ev(KeyCode::Char('c'), KeyModifiers::empty())));
        // A different code should not match.
        assert!(!b.matches(&ev(KeyCode::Char('x'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn matches_ignores_shift_for_unmodified_binding() {
        // Pressing 'G' arrives as Char('G') + SHIFT; a "G" binding must match.
        let b = parse_binding("G").unwrap();
        assert!(b.matches(&ev(KeyCode::Char('G'), KeyModifiers::SHIFT)));
    }

    #[test]
    fn unmodified_binding_rejects_ctrl_press() {
        let b = parse_binding("g").unwrap();
        assert!(!b.matches(&ev(KeyCode::Char('g'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn pressed_matches_any_in_list() {
        let binds = bind(&["Up", "k"]);
        assert!(pressed(&binds, &ev(KeyCode::Up, KeyModifiers::empty())));
        assert!(pressed(
            &binds,
            &ev(KeyCode::Char('k'), KeyModifiers::empty())
        ));
        assert!(!pressed(
            &binds,
            &ev(KeyCode::Char('j'), KeyModifiers::empty())
        ));
    }

    #[test]
    fn default_keymap_has_expected_bindings() {
        let km = Keymap::default();
        assert!(pressed(
            &km.quit,
            &ev(KeyCode::Char('q'), KeyModifiers::empty())
        ));
        assert!(pressed(
            &km.quit,
            &ev(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
        assert!(pressed(
            &km.switch_panel,
            &ev(KeyCode::Tab, KeyModifiers::empty())
        ));
        assert!(pressed(
            &km.toggle_hidden,
            &ev(KeyCode::Char('.'), KeyModifiers::ALT)
        ));
    }

    #[test]
    fn config_uses_serde_defaults_for_missing_fields() {
        // Only one field set; the rest must fall back to defaults.
        let cfg: Config = toml::from_str("tree_width = 42").unwrap();
        assert_eq!(cfg.tree_width, 42);
        assert!(!cfg.show_hidden);
        assert!(pressed(
            &cfg.keys.quit,
            &ev(KeyCode::Char('q'), KeyModifiers::empty())
        ));
    }

    #[test]
    fn config_rejects_invalid_key_spec() {
        let result: Result<Config, _> = toml::from_str("[keys]\nquit = [\"nope\"]");
        assert!(result.is_err());
    }

    #[test]
    fn default_config_serializes_and_round_trips() {
        let cfg = Config::default();
        let toml = toml::to_string_pretty(&cfg).expect("default config must serialize");
        assert!(!toml.is_empty());
        let back: Config = toml::from_str(&toml).expect("serialized config must round-trip");
        assert_eq!(back.show_hidden, cfg.show_hidden);
        assert_eq!(back.git_mode, cfg.git_mode);
        assert_eq!(back.tree_width, cfg.tree_width);
        assert!(pressed(
            &back.keys.quit,
            &ev(KeyCode::Char('q'), KeyModifiers::empty())
        ));
    }

    #[test]
    fn theme_config_with_name_serializes_and_round_trips() {
        use crate::theme::ThemeConfig;
        let mut cfg = Config::default();
        cfg.theme = ThemeConfig::from_preset("monokai");
        let toml = toml::to_string_pretty(&cfg).expect("must serialize");
        assert!(toml.contains("monokai"), "theme name must appear in TOML:\n{toml}");
        let back: Config = toml::from_str(&toml).expect("must round-trip");
        let theme = back.theme.resolve();
        let expected = crate::theme::Theme::preset("monokai").unwrap();
        assert_eq!(theme.accent, expected.accent, "theme must be restored from name");
    }

    #[test]
    fn load_returns_path_of_loaded_file() {
        let dir = std::env::temp_dir().join(format!(
            "tv_cfg_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg_file = dir.join("tv.toml");
        std::fs::write(&cfg_file, "tree_width = 99\n").unwrap();

        let (cfg, path) = load(&dir);
        assert_eq!(cfg.tree_width, 99);
        assert_eq!(path.as_deref(), Some(cfg_file.as_path()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_and_reload_preserves_theme() {
        let dir = std::env::temp_dir().join(format!(
            "tv_theme_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // Write a config without a theme (like a project-local tv.toml).
        std::fs::write(dir.join("tv.toml"), "tree_width = 30\n").unwrap();

        let (mut cfg, path) = load(&dir);
        assert_eq!(cfg.tree_width, 30);

        // Simulate picking "synthwave84" from the theme picker.
        use crate::theme::ThemeConfig;
        cfg.theme = ThemeConfig::from_preset("synthwave84");
        save(&cfg, path.as_deref().unwrap());

        // Reload and verify the theme survived.
        let (reloaded, _) = load(&dir);
        let theme = reloaded.theme.resolve();
        let expected = crate::theme::Theme::preset("synthwave84").unwrap();
        assert_eq!(
            theme.accent, expected.accent,
            "theme must survive a save/reload cycle"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn config_paths_are_local_first_then_global() {
        let root = Path::new("/a/b/c");
        let paths = config_paths(root);
        // Project-local: root first, then each ancestor.
        assert_eq!(paths[0], PathBuf::from("/a/b/c/tv.toml"));
        assert_eq!(paths[1], PathBuf::from("/a/b/tv.toml"));
        assert_eq!(paths[2], PathBuf::from("/a/tv.toml"));
        assert_eq!(paths[3], PathBuf::from("/tv.toml"));
        // Global config (if resolvable) comes after all local candidates.
        if let Some(global) = global_config_path() {
            assert_eq!(*paths.last().unwrap(), global);
            assert!(paths.iter().position(|p| *p == global).unwrap() >= 4);
        }
    }
}
