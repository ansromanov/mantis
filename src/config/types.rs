//! Config struct definitions, serde defaults, and the one-time legacy-field migration.
//!
//! Every user-tunable option is represented here with `#[serde(default)]` so
//! partial configs and older files still load cleanly. The embedded
//! `mantis.toml` is the fully-commented source of truth; this module provides
//! the Rust data model for grouped settings such as tree, content, tabs, and UI.
//! New fields must match their default in `Config::default()` so sparse saves
//! round-trip without emitting unchanged defaults.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::app::DiffMode;
use crate::plugin::PluginEntry;
use crate::theme::ThemeConfig;

use super::keymap::Keymap;

/// General application configuration, grouped under `[general]` in the TOML.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct GeneralConfig {
    /// External editor command. Overrides `$VISUAL` and `$EDITOR`. When unset
    /// (the default), falls back to `$VISUAL` then `$EDITOR`, then probes for
    /// `nano` before falling back to `vim` on Unix / `notepad` on Windows.
    pub editor: Option<String>,
}

impl GeneralConfig {
    /// A fully-populated instance for config-validation schema building.
    pub(crate) fn schema() -> Self {
        GeneralConfig {
            editor: Some("code --wait".into()),
        }
    }
}

/// Tree-pane configuration, grouped under `[tree]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TreeConfig {
    /// Show dotfiles / hidden entries in the tree.
    pub show_hidden: bool,
    /// Tree pane width as a percentage of terminal width (5-95).
    pub width: u16,
    /// PageUp/Down scroll the tree viewport without moving selection.
    pub independent_scroll: bool,
    /// Draw indentation guides (│) in the tree pane.
    pub indent_guides: bool,
    /// Nerd Font file-type icons in the tree.
    pub icons: bool,
}

impl Default for TreeConfig {
    fn default() -> Self {
        TreeConfig {
            show_hidden: false,
            width: 20,
            independent_scroll: false,
            indent_guides: true,
            icons: false,
        }
    }
}

/// Content-pane configuration, grouped under `[content]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct ContentConfig {
    /// Enable word wrapping in the content pane.
    pub word_wrap: bool,
    /// Show line-number gutter in the content pane.
    pub line_numbers: bool,
    /// Show a scrollbar in the content pane.
    pub scrollbar: bool,
    /// Show scroll percentage indicator.
    pub scroll_percentage: bool,
    /// Auto-reload file content when it changes on disk.
    pub watch: bool,
    /// Show encoding and line-ending info in the status bar.
    pub show_file_info: bool,
    /// Max file size (bytes) for JSON/YAML pretty-printing and fold detection.
    /// Files exceeding this limit are shown as raw text via memory-mapped I/O
    /// instead of being pretty-printed or parsed for fold regions.
    pub prettify_size_limit: usize,
    /// Auto-enter follow when opening a growing `.log`.
    pub follow_tail_auto: bool,
    /// Enable log level and timestamp colorizing.
    pub log_colorizing: bool,
    /// Mask credential-shaped file values in the content pane by default.
    pub mask_secrets: bool,
    /// Render image files inline in the content pane when the terminal supports
    /// the Kitty graphics protocol (Ghostty, Kitty, WezTerm, Konsole). When
    /// `false`, or on any other terminal, the text placeholder is shown instead.
    pub image_preview: bool,
}

impl Default for ContentConfig {
    fn default() -> Self {
        ContentConfig {
            word_wrap: false,
            line_numbers: true,
            scrollbar: true,
            scroll_percentage: true,
            watch: false,
            show_file_info: true,
            prettify_size_limit: 10 * 1024 * 1024,
            follow_tail_auto: false,
            log_colorizing: true,
            mask_secrets: true,
            image_preview: true,
        }
    }
}

/// Search/filter configuration, grouped under `[search]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    /// Enable in-file incremental search via `/`.
    pub in_file_search: bool,
    /// Trailing context lines shown after each content match.
    pub context_lines: usize,
    /// Restore last search query when reopening search.
    pub keep_query: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            in_file_search: true,
            context_lines: 0,
            keep_query: false,
        }
    }
}

/// Menu-bar configuration, grouped under `[ui]` in the TOML.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    /// Keep the action menu row visible even when its dropdown is closed.
    pub menu_bar: bool,
}

/// Git-related configuration, grouped under `[git]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct GitConfig {
    /// Show git status colours/markers in the tree.
    pub status: bool,
    /// Include deleted files in git status.
    pub show_deleted: bool,
    /// Include untracked (??) files in git status/colours.
    pub show_untracked: bool,
    /// Include ignored (!!) files in git status/colours.
    pub show_ignored: bool,
    /// Respect .gitignore when listing files (was top-level `ignore_gitignore`).
    pub ignore_gitignore: bool,
    /// Diff-view defaults.
    pub diff: GitDiffConfig,
}

impl Default for GitConfig {
    fn default() -> Self {
        GitConfig {
            status: true,
            show_deleted: false,
            show_untracked: true,
            show_ignored: false,
            ignore_gitignore: false,
            diff: GitDiffConfig::default(),
        }
    }
}

/// Defaults for the built-in diff view, nested under `[git.diff]`.
#[derive(Default, Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct GitDiffConfig {
    /// Default active diff source: `"all"` | `"staged"` | `"unstaged"`.
    pub mode: DiffMode,
    /// Start the diff view in side-by-side layout.
    pub side_by_side: bool,
}

/// Status-bar config, grouped under `[statusbar]` in the TOML.
///
/// ## Semantics
/// - **Both `None`** — default mode: all segments visible, using the historical
///   default right-side set `["lnum", "type", "git", "version"]`.
/// - **Either `Some`** — explicit allowlist mode: only segments whose id appears
///   in `left` or `right` are rendered, in the order specified by the list.
///   Unlisted segments are hidden. Both lists empty → empty bar.
///
/// `separator` is the divider drawn between segments (and before the first
/// segment of each row); its default, a single space, reproduces the historical
/// rendering byte-for-byte. `colors` maps a segment id to a theme color-role
/// name (see `crate::theme::THEME_COLOR_ROLES`) to override that segment's
/// foreground. `height` selects a one-row (default) or two-row bar; in two-row
/// mode the left-aligned segments fill the top row and right-aligned segments
/// the bottom row, with elision applied per row.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct StatusBarConfig {
    /// Segments to show on the left, in order. `None` = default behaviour.
    /// Valid ids: see [`StatusBarConfig::VALID_SEGMENTS`].
    pub left: Option<Vec<String>>,
    /// Segments to show on the right, in order. `None` = default behaviour.
    /// Valid ids: see [`StatusBarConfig::VALID_SEGMENTS`].
    pub right: Option<Vec<String>>,
    /// Divider drawn between segments. Default `" "` reproduces the historical
    /// single-space separation.
    pub separator: String,
    /// Per-segment theme color-role overrides, keyed by segment id. The value
    /// is a semantic role name (e.g. `"accent"`, `"git_dirty"`), not a literal
    /// color, so overrides follow the active theme.
    pub colors: HashMap<String, String>,
    /// Status-bar height in rows: `1` (default) or `2`. Two rows spreads the
    /// segments instead of eliding them on narrow terminals.
    pub height: u8,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        StatusBarConfig {
            left: None,
            right: None,
            separator: " ".to_string(),
            colors: HashMap::new(),
            height: 1,
        }
    }
}

impl StatusBarConfig {
    /// Every valid status-bar segment id, in canonical (build) order. This is
    /// the source of truth for schema building and value validation; the
    /// `StatusSegment::id_str` mapping in `src/ui/statusbar/mod.rs` must agree.
    pub(crate) const VALID_SEGMENTS: &[&str] = &[
        "badges",
        "worktrees",
        "scroll",
        "lnum",
        "type",
        "jsonpath",
        "fileinfo",
        "git",
        "errors",
        "folds",
        "pluginfacts",
        "message",
        "pluginerror",
        "version",
        "update",
    ];

    /// A fully-populated instance for config-validation schema building.
    /// Every field is `Some`/populated so that TOML serialization emits them
    /// (unlike `Default` where they are `None`/empty and would be omitted).
    /// `colors` is seeded with every valid segment id so the schema also
    /// flags unknown segment ids under `[statusbar.colors]`.
    pub(crate) fn schema() -> Self {
        StatusBarConfig {
            left: Some(vec!["hint".into()]),
            right: Some(vec!["version".into()]),
            separator: " ".to_string(),
            colors: Self::VALID_SEGMENTS
                .iter()
                .map(|id| (id.to_string(), "accent".to_string()))
                .collect(),
            height: 1,
        }
    }
}

/// Update-checking configuration, grouped under `[updates]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct UpdatesConfig {
    /// Enable checking for newer releases.
    pub check: bool,
}

impl Default for UpdatesConfig {
    fn default() -> Self {
        UpdatesConfig { check: true }
    }
}

/// Local telemetry configuration, grouped under `[telemetry]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq, Default)]
#[serde(default)]
pub struct TelemetryConfig {
    /// Collect anonymous usage events into a local, size-capped log under the
    /// state directory. Nothing is ever sent anywhere. Disabled by default.
    pub enabled: bool,
    /// Whether the telemetry collection notice has been shown to the user.
    #[serde(default)]
    pub notice_shown: bool,
}

/// Tabs/workspace configuration, grouped under `[tabs]` in the TOML.
#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct TabsConfig {
    /// When launching with no path arguments, reopen the tabs (project roots)
    /// that were open at last quit instead of opening the current directory.
    /// Default: true.
    pub restore_on_launch: bool,
}

impl Default for TabsConfig {
    fn default() -> Self {
        TabsConfig {
            restore_on_launch: true,
        }
    }
}

/// The root mantis configuration object.
///
/// Every field carries `#[serde(default)]` so a partial TOML file or an older
/// version with fewer keys still deserialises cleanly. New fields **must** be
/// added here with a `Config::default()` entry that matches their serde default,
/// and a corresponding key in the embedded `mantis.toml`.
#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Grouped general settings.
    pub general: GeneralConfig,
    /// Grouped tree settings.
    pub tree: TreeConfig,
    /// Grouped content settings.
    pub content: ContentConfig,
    /// Grouped search settings.
    pub search: SearchConfig,
    /// Maximum number of recently opened files to remember. Defaults to 10.
    pub recent_files_count: usize,
    pub keys: Keymap,
    pub theme: ThemeConfig,
    /// Pin the most-recently-used command at the top of the command palette's
    /// empty-query view. Default: true.
    pub palette_pin_recent: bool,
    /// Number of most-frequently-used commands to pin below the recent one in
    /// the command palette's empty-query view. 0 disables. Default: 3.
    pub palette_frequent_count: usize,
    /// Per-plugin entries registered in `[plugins]`.
    #[serde(default)]
    pub plugins: HashMap<String, PluginEntry>,
    /// Grouped git settings.
    pub git: GitConfig,
    /// Status-bar segment alignment config.
    pub statusbar: StatusBarConfig,
    /// Grouped update settings.
    pub updates: UpdatesConfig,
    /// Grouped local-telemetry settings.
    pub telemetry: TelemetryConfig,
    /// Grouped tabs/workspace settings.
    pub tabs: TabsConfig,
    /// User-interface chrome settings.
    pub ui: UiConfig,

    // --- deprecated flat keys (read for backward-compat; never written) ---
    #[serde(default, skip_serializing, rename = "git_status")]
    pub legacy_git_status: Option<bool>,
    #[serde(default, skip_serializing, rename = "git_show_deleted")]
    pub legacy_git_show_deleted: Option<bool>,
    #[serde(default, skip_serializing, rename = "git_show_untracked")]
    pub legacy_git_show_untracked: Option<bool>,
    #[serde(default, skip_serializing, rename = "git_show_ignored")]
    pub legacy_git_show_ignored: Option<bool>,
    #[serde(default, skip_serializing, rename = "ignore_gitignore")]
    pub legacy_ignore_gitignore: Option<bool>,
    #[serde(default, skip_serializing, rename = "diff_mode")]
    pub legacy_diff_mode: Option<String>,

    // --- deprecated tree/content/search flat keys ---
    #[serde(default, skip_serializing, rename = "show_hidden")]
    pub legacy_show_hidden: Option<bool>,
    #[serde(default, skip_serializing, rename = "tree_width")]
    pub legacy_tree_width: Option<u16>,
    #[serde(default, skip_serializing, rename = "tree_independent_scroll")]
    pub legacy_tree_independent_scroll: Option<bool>,
    #[serde(default, skip_serializing, rename = "indent_guides")]
    pub legacy_indent_guides: Option<bool>,
    #[serde(default, skip_serializing, rename = "icons")]
    pub legacy_icons: Option<bool>,
    #[serde(default, skip_serializing, rename = "word_wrap")]
    pub legacy_word_wrap: Option<bool>,
    #[serde(default, skip_serializing, rename = "line_numbers")]
    pub legacy_line_numbers: Option<bool>,
    #[serde(default, skip_serializing, rename = "scrollbar")]
    pub legacy_scrollbar: Option<bool>,
    #[serde(default, skip_serializing, rename = "scroll_percentage")]
    pub legacy_scroll_percentage: Option<bool>,
    #[serde(default, skip_serializing, rename = "watch")]
    pub legacy_watch: Option<bool>,
    #[serde(default, skip_serializing, rename = "show_file_info")]
    pub legacy_show_file_info: Option<bool>,
    #[serde(default, skip_serializing, rename = "in_file_search")]
    pub legacy_in_file_search: Option<bool>,
    #[serde(default, skip_serializing, rename = "search_context_lines")]
    pub legacy_search_context_lines: Option<usize>,
    #[serde(default, skip_serializing, rename = "keep_search_query")]
    pub legacy_keep_search_query: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            general: GeneralConfig::default(),
            tree: TreeConfig::default(),
            content: ContentConfig::default(),
            search: SearchConfig::default(),
            recent_files_count: 10,
            keys: Keymap::default(),
            theme: ThemeConfig::default(),
            palette_pin_recent: true,
            palette_frequent_count: 3,
            plugins: HashMap::new(),
            git: GitConfig::default(),
            statusbar: StatusBarConfig::default(),
            updates: UpdatesConfig::default(),
            telemetry: TelemetryConfig::default(),
            tabs: TabsConfig::default(),
            ui: UiConfig::default(),

            legacy_git_status: None,
            legacy_git_show_deleted: None,
            legacy_git_show_untracked: None,
            legacy_git_show_ignored: None,
            legacy_ignore_gitignore: None,
            legacy_diff_mode: None,
            legacy_show_hidden: None,
            legacy_tree_width: None,
            legacy_tree_independent_scroll: None,
            legacy_indent_guides: None,
            legacy_icons: None,
            legacy_word_wrap: None,
            legacy_line_numbers: None,
            legacy_scrollbar: None,
            legacy_scroll_percentage: None,
            legacy_watch: None,
            legacy_show_file_info: None,
            legacy_in_file_search: None,
            legacy_search_context_lines: None,
            legacy_keep_search_query: None,
        }
    }
}

impl Config {
    /// Folds any set legacy top-level git keys into `self.git`, then clears them
    /// so they are never re-serialized. New `[git]` keys win only when the legacy
    /// key is absent (old files keep their meaning until the user re-saves).
    /// Folds legacy top-level tree/content/search keys into their respective
    /// grouped tables, then clears them so they are never re-serialized.
    pub fn migrate_legacy_flat_fields(&mut self) {
        if let Some(v) = self.legacy_show_hidden.take() {
            self.tree.show_hidden = v;
        }
        if let Some(v) = self.legacy_tree_width.take() {
            self.tree.width = v;
        }
        if let Some(v) = self.legacy_tree_independent_scroll.take() {
            self.tree.independent_scroll = v;
        }
        if let Some(v) = self.legacy_indent_guides.take() {
            self.tree.indent_guides = v;
        }
        if let Some(v) = self.legacy_icons.take() {
            self.tree.icons = v;
        }
        if let Some(v) = self.legacy_word_wrap.take() {
            self.content.word_wrap = v;
        }
        if let Some(v) = self.legacy_line_numbers.take() {
            self.content.line_numbers = v;
        }
        if let Some(v) = self.legacy_scrollbar.take() {
            self.content.scrollbar = v;
        }
        if let Some(v) = self.legacy_scroll_percentage.take() {
            self.content.scroll_percentage = v;
        }
        if let Some(v) = self.legacy_watch.take() {
            self.content.watch = v;
        }
        if let Some(v) = self.legacy_show_file_info.take() {
            self.content.show_file_info = v;
        }
        if let Some(v) = self.legacy_in_file_search.take() {
            self.search.in_file_search = v;
        }
        if let Some(v) = self.legacy_search_context_lines.take() {
            self.search.context_lines = v;
        }
        if let Some(v) = self.legacy_keep_search_query.take() {
            self.search.keep_query = v;
        }
    }

    /// Folds deprecated top-level git flat keys into `self.git`, then clears them.
    pub fn migrate_legacy_git_fields(&mut self) {
        if let Some(v) = self.legacy_git_status.take() {
            self.git.status = v;
        }
        if let Some(v) = self.legacy_git_show_deleted.take() {
            self.git.show_deleted = v;
        }
        if let Some(v) = self.legacy_git_show_untracked.take() {
            self.git.show_untracked = v;
        }
        if let Some(v) = self.legacy_git_show_ignored.take() {
            self.git.show_ignored = v;
        }
        if let Some(v) = self.legacy_ignore_gitignore.take() {
            self.git.ignore_gitignore = v;
        }
        if let Some(v) = self.legacy_diff_mode.take() {
            // Attempt to parse legacy diff_mode string into DiffMode.
            // If it doesn't match, serde would reject it; fall back to default.
            match v.as_str() {
                "staged" => self.git.diff.mode = DiffMode::Staged,
                "unstaged" => self.git.diff.mode = DiffMode::Unstaged,
                _ => {}
            }
        }
    }

    /// Migrates old process plugin paths (e.g. `mantis-plugin-markdown` -> `markdown`)
    /// on load, keeping any custom config like `enabled` intact.
    pub fn migrate_legacy_plugin_paths(&mut self) {
        // 1. Rename any plugins map keys from "mantis-plugin-*" to bare names
        let old_keys: Vec<String> = self
            .plugins
            .keys()
            .filter(|k| *k == "mantis-plugin-markdown" || *k == "mantis-plugin-iconize")
            .cloned()
            .collect();
        for old_key in old_keys {
            if let Some(entry) = self.plugins.remove(&old_key) {
                let new_key = if old_key == "mantis-plugin-markdown" {
                    "markdown".to_string()
                } else {
                    "iconize".to_string()
                };
                self.plugins.entry(new_key).or_insert(entry);
            }
        }

        // 2. Migrate the path inside each entry
        for entry in self.plugins.values_mut() {
            if let Some(fname) = entry.path.file_name().and_then(|s| s.to_str()) {
                if fname == "mantis-plugin-markdown" {
                    entry.path.set_file_name("markdown");
                } else if fname == "mantis-plugin-markdown.exe" {
                    entry.path.set_file_name("markdown.exe");
                } else if fname == "mantis-plugin-iconize" {
                    entry.path.set_file_name("iconize");
                } else if fname == "mantis-plugin-iconize.exe" {
                    entry.path.set_file_name("iconize.exe");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "types_test.rs"]
mod tests;
