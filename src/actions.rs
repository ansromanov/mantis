//! The single action registry: one canonical list every UI surface derives from.
//!
//! Historically the Ctrl-P command palette (`command_palette::COMMANDS`), the
//! keymap's `bindings_for_action` match, and the `?` help overlay
//! (`ui::popups::help`) each hand-maintained their own list of action ids,
//! and the three drifted — a key could be bound but missing from the palette,
//! or the palette could offer an id the keymap didn't recognise. `ACTIONS`
//! fixes that: it is the one place an action's canonical `id`, its palette
//! display name (if any), and its help-overlay placement (if any) are
//! declared. `command_palette::COMMANDS` filters this list for
//! `palette.is_some()`; `ui::popups::help` groups it by `.help`'s section
//! name. `Keymap::bindings_for_action` and
//! `app::key_handlers::editor::dispatch_command` both match on these same
//! canonical ids — see `actions_test.rs` for the parity test that keeps all
//! three surfaces and the keymap fields in sync.

/// One entry in the action registry.
pub struct ActionSpec {
    /// Canonical action id. This is the single string every surface
    /// (`Keymap::bindings_for_action`, `dispatch_command`, the palette, the
    /// help overlay) uses to refer to this action. Exactly one id per action —
    /// no aliases.
    pub id: &'static str,
    /// `Some(display name)` if this action should appear in the Ctrl-P
    /// command palette; `None` for pure-navigation actions that would only
    /// clutter it.
    pub palette: Option<&'static str>,
    /// `Some((section, description))` if this action should appear in the
    /// `?` help overlay's keymap-driven sections (`Global`, `Tree panel`,
    /// `Content panel`); `None` otherwise. The dedicated Git section in the
    /// help overlay interleaves static orientation rows with a handful of
    /// these same action ids under git-specific phrasing, so some actions
    /// intentionally have `None` here despite appearing in that Git section.
    pub help: Option<(&'static str, &'static str)>,
}

/// The canonical action registry. Add a new bound action here first, then
/// wire its dispatch in `app::key_handlers::editor::dispatch_command` (if
/// palette-invokable) and its default binding in `Keymap::default`.
pub static ACTIONS: &[ActionSpec] = &[
    // -- Global -----------------------------------------------------------
    ActionSpec {
        id: "help",
        palette: Some("Toggle help"),
        help: Some(("Global", "toggle this help")),
    },
    ActionSpec {
        id: "switch_panel",
        palette: None,
        help: Some(("Global", "switch panel")),
    },
    ActionSpec {
        id: "quit",
        palette: Some("Quit"),
        help: Some(("Global", "quit")),
    },
    ActionSpec {
        id: "toggle_hidden",
        palette: Some("Toggle hidden files"),
        help: Some(("Global", "toggle hidden files")),
    },
    ActionSpec {
        id: "theme_picker",
        palette: Some("Open theme picker"),
        help: Some(("Global", "pick a theme")),
    },
    ActionSpec {
        id: "plugin_picker",
        palette: Some("Open plugin manager"),
        help: Some(("Global", "plugin manager")),
    },
    ActionSpec {
        id: "git_mode_toggle",
        palette: Some("Toggle git mode"),
        help: Some(("Global", "toggle git mode (changed files only + diffs)")),
    },
    ActionSpec {
        id: "open_in_editor",
        palette: Some("Open in editor"),
        help: Some(("Global", "open file in $EDITOR")),
    },
    ActionSpec {
        id: "copy_path",
        palette: Some("Copy absolute path"),
        help: Some(("Global", "copy absolute path to clipboard")),
    },
    ActionSpec {
        id: "copy_relative_path",
        palette: Some("Copy relative path"),
        help: Some(("Global", "copy path relative to tree root to clipboard")),
    },
    ActionSpec {
        id: "recent_files",
        palette: Some("Recent files"),
        help: Some(("Global", "recent files picker")),
    },
    ActionSpec {
        id: "git_mode_flat_toggle",
        palette: Some("Toggle git flat mode"),
        help: Some(("Global", "toggle git flat/tree view (in git mode)")),
    },
    // -- Tree panel ---------------------------------------------------------
    ActionSpec {
        id: "nav_up",
        palette: None,
        help: Some(("Tree panel", "move up")),
    },
    ActionSpec {
        id: "nav_down",
        palette: None,
        help: Some(("Tree panel", "move down")),
    },
    ActionSpec {
        id: "tree_expand",
        palette: None,
        help: Some(("Tree panel", "expand dir / open file")),
    },
    ActionSpec {
        id: "tree_collapse",
        palette: None,
        help: Some(("Tree panel", "collapse dir")),
    },
    ActionSpec {
        id: "tree_up_dir",
        palette: Some("Go up one directory"),
        help: Some(("Tree panel", "go up one directory")),
    },
    ActionSpec {
        id: "tree_collapse_all",
        palette: Some("Collapse all directories"),
        help: Some(("Tree panel", "collapse all directories")),
    },
    ActionSpec {
        id: "tree_expand_all",
        palette: Some("Expand all directories"),
        help: Some(("Tree panel", "expand all directories")),
    },
    ActionSpec {
        id: "find_files",
        palette: Some("Find files"),
        help: Some(("Tree panel", "global fuzzy file-name picker")),
    },
    ActionSpec {
        id: "search_files",
        palette: Some("Open file search"),
        help: Some(("Tree panel", "tree filter / in-file search")),
    },
    ActionSpec {
        id: "search_content",
        palette: Some("Open content search"),
        help: Some(("Tree panel", "fuzzy content search")),
    },
    ActionSpec {
        id: "reload",
        palette: Some("Reload"),
        help: Some(("Tree panel", "reload tree")),
    },
    // -- Content panel --------------------------------------------------
    // nav_up/nav_down are shared bindings: they also scroll the content
    // panel when it is focused. That second meaning is rendered as two
    // hand-written extra rows in the Content panel help section (see
    // `ui::popups::help`) rather than a second ACTIONS entry, since an
    // action id maps to exactly one `help` slot here.
    ActionSpec {
        id: "content_page_up",
        palette: None,
        help: Some(("Content panel", "page scroll")),
    },
    ActionSpec {
        id: "content_page_down",
        palette: None,
        help: Some(("Content panel", "page scroll")),
    },
    ActionSpec {
        id: "content_left",
        palette: None,
        help: Some(("Content panel", "horizontal scroll")),
    },
    ActionSpec {
        id: "content_right",
        palette: None,
        help: Some(("Content panel", "horizontal scroll")),
    },
    ActionSpec {
        id: "content_reset_col",
        palette: None,
        help: Some(("Content panel", "reset horizontal scroll")),
    },
    ActionSpec {
        id: "content_top",
        palette: None,
        help: Some(("Content panel", "go to top")),
    },
    ActionSpec {
        id: "content_bottom",
        palette: None,
        help: Some(("Content panel", "go to bottom")),
    },
    ActionSpec {
        id: "toggle_wrap",
        palette: Some("Toggle word wrap"),
        help: Some(("Content panel", "toggle word wrap")),
    },
    ActionSpec {
        id: "toggle_line_numbers",
        palette: Some("Toggle line numbers"),
        help: Some(("Content panel", "toggle line numbers")),
    },
    ActionSpec {
        id: "toggle_blame",
        palette: Some("Toggle blame"),
        help: Some(("Content panel", "toggle git blame gutter")),
    },
    ActionSpec {
        id: "file_history",
        palette: Some("Open file history"),
        help: Some(("Content panel", "git history of current file")),
    },
    ActionSpec {
        id: "toggle_diff_side_by_side",
        palette: Some("Toggle side-by-side diff"),
        help: Some(("Content panel", "toggle side-by-side diff (in a diff)")),
    },
    ActionSpec {
        id: "diff_hunk_next",
        palette: Some("Next diff hunk"),
        help: Some(("Content panel", "next / previous hunk (in a diff)")),
    },
    ActionSpec {
        id: "diff_hunk_prev",
        palette: Some("Previous diff hunk"),
        help: Some(("Content panel", "next / previous hunk (in a diff)")),
    },
    ActionSpec {
        id: "fold_toggle",
        palette: Some("Toggle fold at cursor"),
        help: Some(("Content panel", "toggle fold at cursor")),
    },
    ActionSpec {
        id: "toggle_raw_markdown",
        palette: Some("Toggle markdown render (markdown plugin)"),
        help: Some(("Content panel", "toggle markdown render (md files)")),
    },
    // -- Bound actions with no keymap-section help row -------------------
    // (either purely a Git-section row, sourced from these same ids with
    // git-specific phrasing, or currently undocumented in the help overlay —
    // unchanged from before this registry existed.)
    ActionSpec {
        id: "toggle_pretty_json",
        palette: Some("Toggle JSON pretty-print"),
        help: None,
    },
    ActionSpec {
        id: "blame_line",
        palette: Some("Blame active line"),
        help: None,
    },
    ActionSpec {
        id: "toggle_diff_staged",
        palette: Some("Cycle diff source (staged/unstaged)"),
        help: None,
    },
    ActionSpec {
        id: "command_palette",
        palette: None,
        help: None,
    },
    ActionSpec {
        id: "toggle_watch",
        palette: Some("Toggle auto watch (reload on file change)"),
        help: None,
    },
    ActionSpec {
        id: "goto_line",
        palette: Some("Go to line"),
        help: None,
    },
    // -- Palette/menu-only actions: no keymap binding --------------------
    ActionSpec {
        id: "open_config_in_editor",
        palette: Some("Open config in editor"),
        help: None,
    },
    ActionSpec {
        id: "show_about",
        palette: Some("About mantis"),
        help: None,
    },
    ActionSpec {
        id: "fold_all",
        palette: Some("Fold all"),
        help: None,
    },
    ActionSpec {
        id: "unfold_all",
        palette: Some("Unfold all"),
        help: None,
    },
];

#[cfg(test)]
#[path = "actions_test.rs"]
mod tests;
