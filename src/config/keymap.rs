//! Keymap types and keybinding parsing.
//!
//! Defines `KeyBinding` (a single key combination), `Keymap` (the full set of
//! action→binding mappings), and the parsing/matching machinery (`pressed`,
//! `bind`, `parse_binding`). Keybinding strings follow the convention
//! `[modifier+]key` where modifier is `ctrl`, `alt`, `cmd`/`super`, and key is
//! a single character (preserving case) or a named key like `Enter`, `Up`,
//! `PageDown`. Serde deserialization of `KeyBinding` goes through
//! `parse_binding`; the `Keymap::default()` provides the shipped bindings.
//! The `matches` method on `KeyBinding` handles the kitty keyboard protocol's
//! alternate-key reporting for layout-independent matching.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(unix)]
use crate::event_source::CURRENT_ALT_KEYS;

/// A single key combination, e.g. `q`, `ctrl+c`, `alt+.`, `cmd+p`, `PageUp`.
#[derive(Clone, Copy)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
}

/// Map a US keyboard base-layout character to its shifted variant.
/// This is the US ANSI keyboard shift mapping for non-letter symbol keys.
/// For ASCII letters, `to_ascii_uppercase` suffices and is handled separately.
fn us_shifted(c: char) -> char {
    match c {
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        '`' => '~',
        c => c,
    }
}

impl KeyBinding {
    /// Whether a key event matches this binding. Shift is intentionally
    /// ignored because crossterm already encodes it in the char case.
    ///
    /// On terminals that support the kitty keyboard protocol with
    /// `REPORT_ALTERNATE_KEYS`, the event carries alternate keycodes — the
    /// **shifted** key (capital/symbol in the current layout) and the
    /// **base-layout** key (the US-physical key). For ASCII alphabetic
    /// bindings the base-layout key is preferred (layout-independent).
    /// For non-letter symbols the base key + US shift mapping is used, so
    /// bindings like `?` (Shift+/ on US) work regardless of keyboard layout.
    /// When only the shifted key is reported (2-field CSI-u), it is used as
    /// a fallback.
    pub fn matches(&self, key: &KeyEvent) -> bool {
        #[cfg(unix)]
        let event_code = if matches!(self.code, KeyCode::Char(_)) {
            let alt = CURRENT_ALT_KEYS.with(|c| c.get());
            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
            if let Some(b) = alt.base.filter(|b| b.is_ascii_alphabetic()) {
                KeyCode::Char(if shift { b.to_ascii_uppercase() } else { b })
            } else if let Some(b) = alt.base {
                KeyCode::Char(if shift { us_shifted(b) } else { b })
            } else if let Some(s) = alt.shifted {
                KeyCode::Char(s)
            } else {
                key.code
            }
        } else {
            key.code
        };
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
    /// Returns the bindings for an `action_id`, or an empty slice.
    fn bindings_for_action(&self, action_id: &str) -> &[KeyBinding] {
        match action_id {
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
            "help" => &self.help,
            "quit" => &self.quit,
            "switch_panel" => &self.switch_panel,
            "search_files" => &self.search_files,
            "find_files" => &self.find_files,
            "search_content" => &self.search_content,
            "file_history" => &self.file_history,
            "theme_picker" => &self.theme_picker,
            "nav_up" => &self.nav_up,
            "nav_down" => &self.nav_down,
            "tree_expand" => &self.tree_expand,
            "tree_collapse" => &self.tree_collapse,
            "content_left" => &self.content_left,
            "content_right" => &self.content_right,
            "content_top" => &self.content_top,
            "content_bottom" => &self.content_bottom,
            "content_page_up" => &self.content_page_up,
            "content_page_down" => &self.content_page_down,
            "content_reset_col" => &self.content_reset_col,
            "diff_hunk_next" => &self.diff_hunk_next,
            "diff_hunk_prev" => &self.diff_hunk_prev,
            "git_mode_toggle" => &self.git_mode_toggle,
            "git_mode_flat_toggle" => &self.git_mode_flat_toggle,
            "command_palette" => &self.command_palette,
            "recent_files" => &self.recent_files,
            "plugin_picker" => &self.plugin_picker,
            "toggle_wrap" => &self.toggle_wrap,
            "goto_line" => &self.goto_line,
            _ => &[],
        }
    }

    /// Returns a display label for the first binding mapped to `action_id`,
    /// e.g. `"Ctrl+G"` for `toggle_git_mode`.
    pub fn label_for_action(&self, action_id: &str) -> String {
        self.bindings_for_action(action_id)
            .first()
            .map(|b| b.display())
            .unwrap_or_default()
    }

    /// Returns all bindings for `action_id` joined by ` / `, e.g. `"q / Ctrl+C"`,
    /// or `"—"` when the action is unbound.
    pub fn labels_for_action(&self, action_id: &str) -> String {
        let bindings = self.bindings_for_action(action_id);
        if bindings.is_empty() {
            return "—".to_string();
        }
        bindings
            .iter()
            .map(|b| b.display())
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

/// The complete set of user-rebindable key mappings.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Keymap {
    // Global
    pub quit: Vec<KeyBinding>,
    pub help: Vec<KeyBinding>,
    pub toggle_hidden: Vec<KeyBinding>,
    pub search_files: Vec<KeyBinding>,
    pub find_files: Vec<KeyBinding>,
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
            find_files: bind(&["ctrl+f"]),
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
pub(crate) fn bind(specs: &[&str]) -> Vec<KeyBinding> {
    specs
        .iter()
        .map(|s| parse_binding(s).expect("invalid default key binding"))
        .collect()
}

pub(crate) fn parse_binding(s: &str) -> Result<KeyBinding, String> {
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
            "shift" => {}
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

#[cfg(test)]
#[path = "keymap_test.rs"]
mod tests;
