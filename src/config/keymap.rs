//! Keymap types and keybinding parsing.
//!
//! Defines `KeyBinding` (a single key combination), `Keymap` (the full set of
//! action→binding mappings), and the parsing/matching machinery (`pressed`,
//! `pressed_in`, `bind`, `parse_binding`). Keybinding strings follow the
//! convention `[scope:][modifier+]key` where scope is `tree` or `content`
//! (restricting the binding to that focused panel; no scope = global),
//! modifier is `ctrl`, `alt`, `cmd`/`super`, and key is a single character
//! (preserving case) or a named key like `Enter`, `Up`, `PageDown`, `F5`.
//! Serde deserialization of `KeyBinding` goes through `parse_binding`; the
//! `Keymap::default()` provides the shipped bindings, which differ between
//! macOS (`cmd+` primaries with `ctrl+` fallbacks) and other platforms.
//! Default single-letter shortcuts are scoped to the tree panel so the
//! content pane's letter keyspace stays free for future editing features;
//! user configs may still bind unscoped letters explicitly.
//! Ctrl+Shift combinations are not supported: kitty reserves `ctrl+shift` as
//! its `kitty_mod` shortcut prefix, Windows Terminal binds Ctrl+Shift+P/F for
//! its own palette and search, and legacy terminals (macOS Terminal.app, plain
//! xterm, SSH) can't distinguish `Ctrl+Shift+Letter` from `Ctrl+Letter` at all.
//! Every default binding therefore uses plain `ctrl+<lowercase letter>`,
//! Shift via char case on unmodified keys, or named keys. Modifier+letter
//! bindings are matched case-insensitively (and `parse_binding` normalizes
//! them to lowercase), so CapsLock, a held Shift, or a config written as
//! `ctrl+P`/`ctrl+shift+p` all resolve to the same `ctrl+p` action.
//! The `matches` method also handles the kitty keyboard protocol's
//! alternate-key reporting for layout-independent matching.
//!
//! `bindings_for_action`/`label_for_action`/`labels_for_action` accept only
//! the canonical action ids declared in `crate::actions::ACTIONS` - one id
//! per `Keymap` field, no aliases. The command palette and the help overlay
//! both look bindings up through these same ids, so keeping this match in
//! sync with `ACTIONS` (enforced by `actions_test.rs`) is what keeps the
//! three surfaces from drifting apart again.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Deserializer, Serialize};

#[cfg(unix)]
use crate::event_source::CURRENT_ALT_KEYS;

/// Which focused panel a binding is active in. `Global` bindings fire
/// everywhere; `Tree`/`Content` bindings only when that panel has focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BindingScope {
    Global,
    Tree,
    Content,
}

/// A single key combination, e.g. `q`, `ctrl+c`, `alt+.`, `cmd+p`, `PageUp`,
/// optionally scoped to a panel: `tree:q`, `content:ctrl+b`.
#[derive(Clone, Copy)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    pub super_key: bool,
    pub scope: BindingScope,
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
        // On Windows, crossterm derives the reported char's case from
        // `shift_pressed XOR capslock_on` (see crossterm's
        // `event::sys::windows::parse`), so with CapsLock on, an unshifted
        // letter arrives uppercase even though the `SHIFT` modifier bit is
        // `false`. Re-derive the case from that modifier bit alone so
        // bindings keep matching regardless of CapsLock state.
        #[cfg(not(unix))]
        let event_code = match key.code {
            KeyCode::Char(c) if c.is_ascii_alphabetic() => {
                let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                KeyCode::Char(if shift {
                    c.to_ascii_uppercase()
                } else {
                    c.to_ascii_lowercase()
                })
            }
            other => other,
        };

        // Modifier+letter bindings match case-insensitively: Ctrl+Shift
        // combos are unsupported (terminals reserve or conflate them), so
        // the char case carries no meaning once a modifier is held — this
        // also makes the bindings immune to CapsLock and a stray Shift.
        let code_matches = match (self.code, event_code) {
            (KeyCode::Char(b), KeyCode::Char(e))
                if (self.ctrl || self.alt || self.super_key) && b.is_ascii_alphabetic() =>
            {
                b.eq_ignore_ascii_case(&e)
            }
            (b, e) => b == e,
        };
        code_matches
            && key.modifiers.contains(KeyModifiers::CONTROL) == self.ctrl
            && key.modifiers.contains(KeyModifiers::ALT) == self.alt
            && key.modifiers.contains(KeyModifiers::SUPER) == self.super_key
    }

    /// Returns a human-readable label for this binding, e.g. `"Ctrl+P"`,
    /// `"Q (tree)"`. Modifier+letter bindings are normalized to lowercase and
    /// matched case-insensitively, so the letter is shown uppercase in the
    /// conventional shortcut style. Panel-scoped bindings get a
    /// `" (tree)"`/`" (content)"` suffix so help/palette surfaces make clear
    /// the key only fires when that panel is focused.
    pub fn display(&self) -> String {
        let has_modifier = self.ctrl || self.alt || self.super_key;
        let key = match self.code {
            KeyCode::Char(' ') => "Space".to_string(),
            KeyCode::Char(c) if has_modifier && c.is_ascii_alphabetic() => {
                c.to_ascii_uppercase().to_string()
            }
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
            KeyCode::F(n) => format!("F{n}"),
            ref other => format!("{other:?}"),
        };
        let base = match (self.ctrl, self.alt, self.super_key) {
            (true, true, false) => format!("Ctrl+Alt+{key}"),
            (true, false, false) => format!("Ctrl+{key}"),
            (false, true, false) => format!("Alt+{key}"),
            (false, false, true) => format!("Cmd+{key}"),
            (true, false, true) => format!("Ctrl+Cmd+{key}"),
            _ => key,
        };
        match self.scope {
            BindingScope::Global => base,
            BindingScope::Tree => format!("{base} (tree)"),
            BindingScope::Content => format!("{base} (content)"),
        }
    }
}

/// Returns true if any binding in the list matches the key event, ignoring
/// binding scopes. Use in modal/overlay contexts where panel focus is not
/// meaningful; the main-view dispatch goes through `pressed_in`.
pub fn pressed(bindings: &[KeyBinding], key: &KeyEvent) -> bool {
    bindings.iter().any(|b| b.matches(key))
}

/// Returns true if any binding in the list matches the key event and is
/// active in `scope`: global bindings always are, panel-scoped bindings only
/// when `scope` is that panel.
pub fn pressed_in(bindings: &[KeyBinding], key: &KeyEvent, scope: BindingScope) -> bool {
    bindings
        .iter()
        .any(|b| (b.scope == BindingScope::Global || b.scope == scope) && b.matches(key))
}

impl Keymap {
    /// Returns the bindings for a canonical `action_id` (see
    /// `crate::actions::ACTIONS`), or an empty slice for an unbound or
    /// palette/menu-only action (e.g. `fold_all`, `show_about`).
    fn bindings_for_action(&self, action_id: &str) -> &[KeyBinding] {
        match action_id {
            "help" => &self.help,
            "toggle_hidden" => &self.toggle_hidden,
            "search_files" => &self.search_files,
            "find_files" => &self.find_files,
            "search_content" => &self.search_content,
            "reload" => &self.reload,
            "switch_panel" => &self.switch_panel,
            "file_history" => &self.file_history,
            "repo_commit_log" => &self.repo_commit_log,
            "theme_picker" => &self.theme_picker,
            "quit" => &self.quit,
            "nav_up" => &self.nav_up,
            "nav_down" => &self.nav_down,
            "tree_expand" => &self.tree_expand,
            "tree_collapse" => &self.tree_collapse,
            "tree_collapse_all" => &self.tree_collapse_all,
            "tree_expand_all" => &self.tree_expand_all,
            "content_left" => &self.content_left,
            "content_right" => &self.content_right,
            "content_top" => &self.content_top,
            "content_bottom" => &self.content_bottom,
            "content_page_up" => &self.content_page_up,
            "content_page_down" => &self.content_page_down,
            "content_reset_col" => &self.content_reset_col,
            "toggle_wrap" => &self.toggle_wrap,
            "toggle_line_numbers" => &self.toggle_line_numbers,
            "toggle_pretty_json" => &self.toggle_pretty_json,
            "toggle_blame" => &self.toggle_blame,
            "blame_line" => &self.blame_line,
            "toggle_diff_side_by_side" => &self.toggle_diff_side_by_side,
            "toggle_diff_staged" => &self.toggle_diff_staged,
            "toggle_file_revision" => &self.toggle_file_revision,
            "blame_open_commit" => &self.blame_open_commit,
            "diff_hunk_next" => &self.diff_hunk_next,
            "diff_hunk_prev" => &self.diff_hunk_prev,
            "git_mode_toggle" => &self.git_mode_toggle,
            "git_mode_flat_toggle" => &self.git_mode_flat_toggle,
            "command_palette" => &self.command_palette,
            "open_in_editor" => &self.open_in_editor,
            "open_external" => &self.open_external,
            "fold_toggle" => &self.fold_toggle,
            "toggle_watch" => &self.toggle_watch,
            "recent_files" => &self.recent_files,
            "copy_path" => &self.copy_path,
            "copy_relative_path" => &self.copy_relative_path,
            "copy_line" => &self.copy_line,
            "copy_file" => &self.copy_file,
            "plugin_picker" => &self.plugin_picker,
            "goto_line" => &self.goto_line,
            "toggle_raw_markdown" => &self.toggle_raw_markdown,
            "tree_width_grow" => &self.tree_width_grow,
            "tree_width_shrink" => &self.tree_width_shrink,
            "tree_up_dir" => &self.tree_up_dir,
            _ => &[],
        }
    }

    pub fn action_for_key(&self, key: &KeyEvent, scope: BindingScope) -> Option<&'static str> {
        for spec in crate::actions::ACTIONS {
            let bindings = self.bindings_for_action(spec.id);
            if crate::config::pressed_in(bindings, key, scope) {
                return Some(spec.id);
            }
        }
        None
    }

    /// Returns a display label for the first binding mapped to `action_id`,
    /// e.g. `"Ctrl+G"` for `git_mode_toggle`.
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

    /// Folds renamed `[keys]` actions from pre-refactor configs into their
    /// current field, then clears the legacy field so it is never re-serialized.
    /// Legacy wins over the new name when both are present, matching
    /// `Config::migrate_legacy_git_fields` (#553).
    pub fn migrate_legacy_keys(&mut self) {
        if let Some(v) = self.legacy_yaml_fold_toggle.take() {
            self.fold_toggle = v;
        }
        if let Some(v) = self.legacy_visual_line_blame.take() {
            self.blame_line = v;
        }
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
    pub repo_commit_log: Vec<KeyBinding>,
    pub theme_picker: Vec<KeyBinding>,
    // Shared navigation (tree + content)
    pub nav_up: Vec<KeyBinding>,
    pub nav_down: Vec<KeyBinding>,
    // Tree panel
    pub tree_expand: Vec<KeyBinding>,
    pub tree_collapse: Vec<KeyBinding>,
    pub tree_collapse_all: Vec<KeyBinding>,
    pub tree_expand_all: Vec<KeyBinding>,
    pub tree_width_grow: Vec<KeyBinding>,
    pub tree_width_shrink: Vec<KeyBinding>,
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
    /// Toggles between diff view and file-at-revision snapshot.
    pub toggle_file_revision: Vec<KeyBinding>,
    /// Opens file at the commit shown on the active blame line.
    pub blame_open_commit: Vec<KeyBinding>,
    pub diff_hunk_next: Vec<KeyBinding>,
    pub diff_hunk_prev: Vec<KeyBinding>,
    pub git_mode_toggle: Vec<KeyBinding>,
    pub git_mode_flat_toggle: Vec<KeyBinding>,
    pub command_palette: Vec<KeyBinding>,
    pub open_in_editor: Vec<KeyBinding>,
    pub open_external: Vec<KeyBinding>,
    pub fold_toggle: Vec<KeyBinding>,
    pub toggle_watch: Vec<KeyBinding>,
    pub recent_files: Vec<KeyBinding>,
    pub copy_path: Vec<KeyBinding>,
    pub copy_relative_path: Vec<KeyBinding>,
    pub copy_line: Vec<KeyBinding>,
    pub copy_file: Vec<KeyBinding>,
    pub plugin_picker: Vec<KeyBinding>,
    pub goto_line: Vec<KeyBinding>,
    pub toggle_raw_markdown: Vec<KeyBinding>,
    pub tree_up_dir: Vec<KeyBinding>,

    // --- deprecated/renamed action keys (read for backward-compat; never written) ---
    /// Old name for `fold_toggle` (#553).
    #[serde(default, skip_serializing, rename = "yaml_fold_toggle")]
    pub legacy_yaml_fold_toggle: Option<Vec<KeyBinding>>,
    /// Old name for `blame_line` (#553).
    #[serde(default, skip_serializing, rename = "visual_line_blame")]
    pub legacy_visual_line_blame: Option<Vec<KeyBinding>>,
}

impl Default for Keymap {
    /// Editor-style defaults (VS Code / Sublime conventions). Single letters
    /// are `tree:`-scoped so the content pane stays letter-free apart from
    /// the vim motion set (`j k h l g G 0 n N`), the copy/blame pairs
    /// (`content:y`/`content:Y`, `content:B`), and `M` (markdown raw/rendered
    /// toggle — bare, unscoped, because the bundled markdown plugin only
    /// recognizes the literal key `M` over its `on_keypress` event; a rebound
    /// key is translated back to `M` before being forwarded, see
    /// `key_handlers::normal::handle_content_key`). All other content-pane
    /// toggles go through modifier combos or the command palette.
    /// No default uses Ctrl+Shift (kitty's `kitty_mod`, Windows Terminal's
    /// own palette/search, indistinguishable from plain Ctrl on legacy
    /// terminals), the Alt modifier (unreliable across terminals), F-keys
    /// beyond the F1/F5 conveniences (poor macOS ergonomics), or the
    /// terminal-critical Ctrl+S/Q/Z/L combinations.
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut map = Keymap {
            quit: bind(&["ctrl+c", "tree:q"]),
            help: bind(&["F1", "?"]),
            toggle_hidden: bind(&["tree:."]),
            search_files: bind(&["/"]),
            find_files: bind(&["ctrl+t"]),
            search_content: bind(&["ctrl+f", "tree:f"]),
            reload: bind(&["ctrl+r", "F5", "tree:r"]),
            switch_panel: bind(&["Tab"]),
            file_history: bind(&["tree:H"]),
            repo_commit_log: bind(&["tree:L"]),
            theme_picker: bind(&["tree:t"]),
            nav_up: bind(&["Up", "k"]),
            nav_down: bind(&["Down", "j"]),
            tree_expand: bind(&["Enter", "Right", "l"]),
            tree_collapse: bind(&["Left", "h"]),
            tree_collapse_all: bind(&["-"]),
            tree_expand_all: bind(&["="]),
            tree_width_grow: bind(&["tree:]"]),
            tree_width_shrink: bind(&["tree:["]),
            content_left: bind(&["Left"]),
            content_right: bind(&["Right"]),
            content_top: bind(&["ctrl+Home", "g", "tree:Home"]),
            content_bottom: bind(&["ctrl+End", "G", "tree:End"]),
            content_page_up: bind(&["PageUp"]),
            content_page_down: bind(&["PageDown"]),
            content_reset_col: bind(&["Home", "0"]),
            toggle_wrap: Vec::new(),
            toggle_line_numbers: Vec::new(),
            toggle_pretty_json: Vec::new(),
            toggle_blame: bind(&["ctrl+b"]),
            blame_line: bind(&["content:B"]),
            toggle_diff_side_by_side: Vec::new(),
            toggle_diff_staged: Vec::new(),
            toggle_file_revision: bind(&["ctrl+u"]),
            blame_open_commit: bind(&["o", "O"]),
            diff_hunk_next: bind(&["n"]),
            diff_hunk_prev: bind(&["N"]),
            git_mode_toggle: bind(&["ctrl+d"]),
            git_mode_flat_toggle: bind(&["tree:F"]),
            command_palette: bind(&["ctrl+p", "tree:P"]),
            open_in_editor: bind(&["ctrl+e", "tree:e"]),
            open_external: bind(&["o"]),
            fold_toggle: bind(&["Space"]),
            toggle_watch: bind(&["tree:W"]),
            recent_files: bind(&["ctrl+o"]),
            copy_path: bind(&["tree:y"]),
            copy_relative_path: bind(&["tree:Y"]),
            copy_line: bind(&["content:y"]),
            copy_file: bind(&["content:Y"]),
            plugin_picker: bind(&["tree:p"]),
            goto_line: bind(&["ctrl+g"]),
            toggle_raw_markdown: bind(&["M"]),
            tree_up_dir: bind(&["Backspace"]),
            legacy_yaml_fold_toggle: None,
            legacy_visual_line_blame: None,
        };
        #[cfg(target_os = "macos")]
        apply_macos_defaults(&mut map);
        map
    }
}

/// Layers the macOS defaults over the base map: `cmd+` primaries with the
/// cross-platform `ctrl+` bindings kept as fallbacks, because
/// Terminal.app/iTerm2 intercept most `cmd+` shortcuts while
/// kitty/WezTerm/Ghostty forward them. `goto_line`/`git_mode_toggle` stay on
/// `ctrl` only.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn apply_macos_defaults(map: &mut Keymap) {
    map.find_files = bind(&["cmd+t", "ctrl+t"]);
    map.command_palette = bind(&["cmd+p", "ctrl+p", "tree:P"]);
    map.search_content = bind(&["cmd+f", "ctrl+f", "tree:f"]);
    map.reload = bind(&["cmd+r", "ctrl+r", "F5", "tree:r"]);
    map.recent_files = bind(&["cmd+o", "ctrl+o"]);
    map.content_top = bind(&["cmd+Up", "ctrl+Home", "g", "tree:Home"]);
    map.content_bottom = bind(&["cmd+Down", "ctrl+End", "G", "tree:End"]);
    map.content_reset_col = bind(&["cmd+Left", "Home", "0"]);
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
    let (scope, spec) = if let Some(rest) = s.strip_prefix("tree:") {
        (BindingScope::Tree, rest)
    } else if let Some(rest) = s.strip_prefix("content:") {
        (BindingScope::Content, rest)
    } else {
        (BindingScope::Global, s)
    };
    let parts: Vec<&str> = spec.split('+').collect();
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

    let mut code = parse_keycode(key[0], s)?;
    // Modifier+letter bindings are case-insensitive (Ctrl+Shift combos are
    // unsupported); normalize to lowercase so `ctrl+P`, `ctrl+shift+p`, and
    // `ctrl+p` in a config all serialize and match identically.
    if ctrl || alt || super_key {
        if let KeyCode::Char(c) = code {
            code = KeyCode::Char(c.to_ascii_lowercase());
        }
    }

    Ok(KeyBinding {
        code,
        ctrl,
        alt,
        super_key,
        scope,
    })
}

fn parse_keycode(s: &str, full: &str) -> Result<KeyCode, String> {
    let mut chars = s.chars();
    if let (Some(c), None) = (chars.next(), chars.clone().next()) {
        return Ok(KeyCode::Char(c));
    }

    let lower = s.to_ascii_lowercase();
    if let Some(n) = lower.strip_prefix('f').and_then(|d| d.parse::<u8>().ok()) {
        if (1..=12).contains(&n) {
            return Ok(KeyCode::F(n));
        }
    }

    let code = match lower.as_str() {
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
            KeyCode::F(n) => format!("F{n}"),
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
        let spec = match self.scope {
            BindingScope::Global => spec,
            BindingScope::Tree => format!("tree:{spec}"),
            BindingScope::Content => format!("content:{spec}"),
        };
        s.serialize_str(&spec)
    }
}

#[cfg(test)]
#[path = "keymap_test.rs"]
mod tests;
