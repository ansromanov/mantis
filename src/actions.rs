//! The single action registry: one canonical list every UI surface derives from.
//!
//! Historically the Ctrl-P command palette (`command_palette::COMMANDS`), the
//! keymap's `bindings_for_action` match, and the `?` help overlay
//! (`ui::popups::help`) each hand-maintained their own list of action ids,
//! and the three drifted — a key could be bound but missing from the palette,
//! or the palette could offer an id the keymap didn't recognise. `ACTIONS`
//! fixes that: it is the one place an action's canonical `id`, its palette
//! display name (if any), and its help description (if any) are
//! declared. `command_palette::COMMANDS` filters this list for
//! `palette.is_some()`; help and the optional menu bar group entries by the
//! shared `.category`.
//! `Keymap::bindings_for_action` and
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
    /// Optional description for an action shown in the help overlay. Help
    /// groups rows by `category`; this field only controls inclusion and text.
    pub help: Option<&'static str>,
    /// Canonical action group shared by help, the command palette, and the menu bar.
    /// The palette prefixes entries with `"{category}: "`.
    pub category: &'static str,
    /// Order in the menu-bar dropdown for this category. `None` excludes the
    /// action from the menu bar while keeping it in the shared taxonomy.
    pub menu: Option<u8>,
    /// Optional one-line description shown dim after the palette entry name.
    /// Makes the palette self-documenting.
    pub description: Option<&'static str>,
}

/// Canonical action groups shared by help, the command palette, and the menu bar.
/// Add a new group here before using it in an action.
pub const ACTION_CATEGORIES: &[&str] = &[
    "General", "View", "Git", "Copy", "Navigate", "Tree", "Tabs", "Safety", "Plugins",
];

/// The canonical action registry. Add a new bound action here first, then
/// wire its dispatch in `app::key_handlers::editor::dispatch_command` (if
/// palette-invokable) and its default binding in `Keymap::default`.
pub static ACTIONS: &[ActionSpec] = &[
    // -- Global -----------------------------------------------------------
    ActionSpec {
        id: "help",
        palette: Some("Toggle help"),
        help: Some("toggle this help"),
        category: "General",
        menu: Some(0),
        description: Some("Toggle the keybinding help overlay"),
    },
    ActionSpec {
        id: "menu_bar",
        palette: Some("Toggle the action menu"),
        help: Some("open the action menu"),
        category: "General",
        menu: Some(9),
        description: Some("Toggle the menu bar"),
    },
    ActionSpec {
        id: "bug_report",
        palette: Some("Report a bug (save diagnostics locally)"),
        help: None,
        category: "General",
        menu: Some(1),
        description: Some("Save diagnostics for a bug report"),
    },
    ActionSpec {
        id: "switch_panel",
        palette: None,
        help: Some("switch panel"),
        category: "General",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "context_menu",
        palette: Some("Open context menu"),
        help: Some("open context menu"),
        category: "General",
        menu: Some(7),
        description: Some("Open the context menu for the focused item"),
    },
    ActionSpec {
        id: "quit",
        palette: Some("Quit"),
        help: Some("quit"),
        category: "General",
        menu: Some(2),
        description: Some("Exit the application"),
    },
    ActionSpec {
        id: "toggle_hidden",
        palette: Some("Toggle hidden files"),
        help: Some("toggle hidden files"),
        category: "General",
        menu: Some(3),
        description: Some("Show or hide dotfiles in the tree"),
    },
    ActionSpec {
        id: "toggle_secret_reveal",
        palette: Some("Toggle secret reveal"),
        help: Some("reveal or mask detected secrets"),
        category: "Safety",
        menu: Some(0),
        description: Some("Temporarily reveal credential-shaped file values"),
    },
    ActionSpec {
        id: "toggle_telemetry",
        palette: Some("Toggle telemetry"),
        help: None,
        category: "General",
        menu: Some(4),
        description: Some("Enable or disable local usage telemetry"),
    },
    ActionSpec {
        id: "theme_picker",
        palette: Some("Open theme picker"),
        help: Some("pick a theme"),
        category: "View",
        menu: Some(0),
        description: Some("Switch the color theme"),
    },
    ActionSpec {
        id: "new_tab",
        palette: Some("Open project as new tab"),
        help: Some("open another project as a new tab"),
        category: "Tabs",
        menu: Some(0),
        description: Some("Type a directory path to open it in a new tab"),
    },
    ActionSpec {
        id: "close_tab",
        palette: Some("Close tab"),
        help: Some("close the current tab"),
        category: "Tabs",
        menu: Some(1),
        description: Some("Close the current tab (kept open if it's the only one)"),
    },
    ActionSpec {
        id: "next_tab",
        palette: Some("Next tab"),
        help: Some("switch to the next tab"),
        category: "Tabs",
        menu: Some(2),
        description: Some("Switch to the next tab"),
    },
    ActionSpec {
        id: "prev_tab",
        palette: Some("Previous tab"),
        help: Some("switch to the previous tab"),
        category: "Tabs",
        menu: Some(3),
        description: Some("Switch to the previous tab"),
    },
    ActionSpec {
        id: "select_tab",
        palette: None,
        help: Some("select tab by number"),
        category: "Tabs",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "move_tab_prev",
        palette: Some("Move tab left"),
        help: Some("move tab left"),
        category: "Tabs",
        menu: Some(4),
        description: Some("Move the current tab one position left"),
    },
    ActionSpec {
        id: "move_tab_next",
        palette: Some("Move tab right"),
        help: Some("move tab right"),
        category: "Tabs",
        menu: Some(5),
        description: Some("Move the current tab one position right"),
    },
    ActionSpec {
        id: "tab_picker",
        palette: Some("Pick an open tab"),
        help: Some("pick an open tab"),
        category: "Tabs",
        menu: Some(6),
        description: Some("Fuzzy-search open tabs by root and branch"),
    },
    ActionSpec {
        id: "reopen_tab",
        palette: Some("Reopen closed tab"),
        help: Some("reopen the most recently closed tab"),
        category: "Tabs",
        menu: Some(7),
        description: Some("Restore the most recently closed tab"),
    },
    ActionSpec {
        id: "plugin_picker",
        palette: Some("Open plugin manager"),
        help: Some("plugin manager"),
        category: "Plugins",
        menu: Some(0),
        description: Some("Manage installed plugins"),
    },
    ActionSpec {
        id: "git_mode_toggle",
        palette: Some("Toggle git mode"),
        help: Some("toggle git mode (changed files only + diffs)"),
        category: "Git",
        menu: Some(0),
        description: Some("Filter tree to changed files and show diffs"),
    },
    ActionSpec {
        id: "open_in_editor",
        palette: Some("Open in editor"),
        help: Some("open file in $EDITOR"),
        category: "General",
        menu: Some(5),
        description: Some("Open the selected file in your editor"),
    },
    ActionSpec {
        id: "open_external",
        palette: Some("Open with default app"),
        help: Some("open file with system default app"),
        category: "General",
        menu: Some(6),
        description: Some("Open the selected file with the system default app"),
    },
    ActionSpec {
        id: "copy_path",
        palette: Some("Copy absolute path"),
        help: Some("copy absolute path to clipboard"),
        category: "Copy",
        menu: Some(0),
        description: Some("Copy the absolute file path to the clipboard"),
    },
    ActionSpec {
        id: "copy_relative_path",
        palette: Some("Copy relative path"),
        help: Some("copy path relative to tree root to clipboard"),
        category: "Copy",
        menu: Some(1),
        description: Some("Copy the relative file path to the clipboard"),
    },
    ActionSpec {
        id: "copy_line",
        palette: Some("Copy line or selection"),
        help: Some("copy current line (or selection if any) to clipboard"),
        category: "Copy",
        menu: Some(2),
        description: Some("Copy the current line or visual selection"),
    },
    ActionSpec {
        id: "copy_file",
        palette: Some("Copy entire file"),
        help: Some("copy entire file content to clipboard"),
        category: "Copy",
        menu: Some(3),
        description: Some("Copy the entire file content to the clipboard"),
    },
    ActionSpec {
        id: "recent_files",
        palette: Some("Recent files"),
        help: Some("recent files picker"),
        category: "Navigate",
        menu: Some(0),
        description: Some("Browse and open recently opened files"),
    },
    ActionSpec {
        id: "toggle_bookmark",
        palette: Some("Toggle bookmark"),
        help: Some("toggle bookmark on current file"),
        category: "Navigate",
        menu: Some(1),
        description: Some("Pin or unpin the current file"),
    },
    ActionSpec {
        id: "bookmarks",
        palette: Some("Bookmarks"),
        help: Some("bookmarks picker"),
        category: "Navigate",
        menu: Some(2),
        description: Some("Browse and open bookmarked files"),
    },
    ActionSpec {
        id: "worktree_picker",
        palette: Some("Open worktree switcher"),
        help: Some("switch git worktree"),
        category: "Git",
        menu: Some(1),
        description: Some("Browse branches and changed-file counts across worktrees"),
    },
    ActionSpec {
        id: "open_worktree_new_tab",
        palette: Some("Open worktree in new tab"),
        help: None,
        category: "Tabs",
        menu: Some(8),
        description: Some("Choose a worktree to open or activate in its own tab"),
    },
    ActionSpec {
        id: "git_mode_flat_toggle",
        palette: Some("Toggle git flat mode"),
        help: Some("toggle git flat/tree view (in git mode)"),
        category: "Git",
        menu: Some(2),
        description: Some("Switch between flat and tree view in git mode"),
    },
    // -- Tree panel ---------------------------------------------------------
    ActionSpec {
        id: "nav_up",
        palette: None,
        help: Some("move up"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "nav_down",
        palette: None,
        help: Some("move down"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "tree_expand",
        palette: None,
        help: Some("expand dir / open file"),
        category: "Tree",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "tree_collapse",
        palette: None,
        help: Some("collapse dir"),
        category: "Tree",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "tree_up_dir",
        palette: Some("Go up one directory"),
        help: Some("go up one directory"),
        category: "Tree",
        menu: Some(0),
        description: Some("Navigate to the parent directory"),
    },
    ActionSpec {
        id: "tree_collapse_all",
        palette: Some("Collapse all directories"),
        help: Some("collapse all directories"),
        category: "Tree",
        menu: Some(1),
        description: Some("Collapse every expanded directory"),
    },
    ActionSpec {
        id: "tree_expand_all",
        palette: Some("Expand all directories"),
        help: Some("expand all directories"),
        category: "Tree",
        menu: Some(2),
        description: Some("Expand every directory in the tree"),
    },
    ActionSpec {
        id: "tree_width_grow",
        palette: Some("Grow tree pane width"),
        help: Some("increase tree pane width"),
        category: "View",
        menu: Some(2),
        description: Some("Increase the tree pane width percentage"),
    },
    ActionSpec {
        id: "tree_width_shrink",
        palette: Some("Shrink tree pane width"),
        help: Some("decrease tree pane width"),
        category: "View",
        menu: Some(3),
        description: Some("Decrease the tree pane width percentage"),
    },
    ActionSpec {
        id: "find_files",
        palette: Some("Find files"),
        help: Some("global fuzzy file-name picker"),
        category: "Tree",
        menu: Some(3),
        description: Some("Fuzzy-find files by name across the whole tree"),
    },
    ActionSpec {
        id: "search_files",
        palette: Some("Open file search"),
        help: Some("tree filter / in-file search"),
        category: "Tree",
        menu: Some(4),
        description: Some("Filter the tree by file name"),
    },
    ActionSpec {
        id: "search_content",
        palette: Some("Open content search"),
        help: Some("fuzzy content search"),
        category: "Tree",
        menu: Some(5),
        description: Some("Search file contents across the whole tree"),
    },
    ActionSpec {
        id: "reload",
        palette: Some("Reload"),
        help: Some("reload tree"),
        category: "Tree",
        menu: Some(6),
        description: Some("Reload the file tree from disk"),
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
        help: Some("page scroll"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_page_down",
        palette: None,
        help: Some("page scroll"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_left",
        palette: None,
        help: Some("horizontal scroll"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_right",
        palette: None,
        help: Some("horizontal scroll"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_reset_col",
        palette: None,
        help: Some("reset horizontal scroll"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_top",
        palette: None,
        help: Some("go to top"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "content_bottom",
        palette: None,
        help: Some("go to bottom"),
        category: "Navigate",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "toggle_wrap",
        palette: Some("Toggle word wrap"),
        help: Some("toggle word wrap"),
        category: "View",
        menu: Some(4),
        description: Some("Toggle word wrapping in the content panel"),
    },
    ActionSpec {
        id: "toggle_line_numbers",
        palette: Some("Toggle line numbers"),
        help: Some("toggle line numbers"),
        category: "View",
        menu: Some(5),
        description: Some("Toggle the line number gutter"),
    },
    ActionSpec {
        id: "toggle_blame",
        palette: Some("Toggle blame"),
        help: Some("toggle git blame gutter"),
        category: "View",
        menu: Some(6),
        description: Some("Toggle the git blame annotation gutter"),
    },
    ActionSpec {
        id: "follow_tail",
        palette: Some("Toggle log tail follow"),
        help: Some("toggle log tail follow"),
        category: "View",
        menu: Some(7),
        description: Some("Pin scroll to bottom of growing logs"),
    },
    ActionSpec {
        id: "filter_lines",
        palette: Some("Filter log lines"),
        help: Some("filter log lines"),
        category: "Navigate",
        menu: Some(3),
        description: Some("Show only lines matching a query"),
    },
    ActionSpec {
        id: "file_history",
        palette: Some("Open file history"),
        help: Some("pick a commit -> view its diff vs your working tree"),
        category: "Git",
        menu: Some(3),
        description: Some("Browse the git log for the current file"),
    },
    ActionSpec {
        id: "repo_commit_log",
        palette: Some("Browse repository commits"),
        help: Some("browse all repository commits, enter to view commit diff"),
        category: "Git",
        menu: Some(4),
        description: Some("Browse all commits in the repository"),
    },
    ActionSpec {
        id: "toggle_diff_side_by_side",
        palette: Some("Toggle side-by-side diff"),
        help: Some("toggle side-by-side diff (in a diff)"),
        category: "Git",
        menu: Some(5),
        description: Some("Switch between unified and side-by-side diff view"),
    },
    ActionSpec {
        id: "diff_hunk_next",
        palette: Some("Next diff hunk"),
        help: Some("next / previous hunk (in a diff)"),
        category: "Git",
        menu: Some(6),
        description: Some("Jump to the next diff hunk"),
    },
    ActionSpec {
        id: "diff_hunk_prev",
        palette: Some("Previous diff hunk"),
        help: Some("next / previous hunk (in a diff)"),
        category: "Git",
        menu: Some(7),
        description: Some("Jump to the previous diff hunk"),
    },
    ActionSpec {
        id: "fold_toggle",
        palette: Some("Toggle fold at cursor"),
        help: Some("toggle fold at cursor"),
        category: "View",
        menu: Some(8),
        description: Some("Fold or unfold the section at the cursor"),
    },
    ActionSpec {
        id: "toggle_raw_markdown",
        palette: Some("Toggle markdown render (markdown plugin)"),
        help: Some("toggle markdown render (md files, markdown plugin)"),
        category: "View",
        menu: Some(9),
        description: Some("Toggle between rendered and raw markdown"),
    },
    // -- Bound actions with no help row row -------------------
    // (either purely a Git-section row, sourced from these same ids with
    // git-specific phrasing, or currently undocumented in the help overlay —
    // unchanged from before this registry existed.)
    ActionSpec {
        id: "toggle_pretty_json",
        palette: Some("Toggle JSON pretty-print"),
        help: None,
        category: "View",
        menu: Some(10),
        description: Some("Toggle JSON pretty-printing in the content panel"),
    },
    ActionSpec {
        id: "toggle_table_view",
        palette: Some("Toggle CSV/TSV table view"),
        help: None,
        category: "View",
        menu: Some(11),
        description: Some("Toggle CSV/TSV table view in the content panel"),
    },
    ActionSpec {
        id: "json_query",
        palette: Some("Open JSON query bar"),
        help: None,
        category: "View",
        menu: Some(12),
        description: Some("Filter or project JSON and JSONL content"),
    },
    ActionSpec {
        id: "blame_line",
        palette: Some("Blame active line"),
        help: Some("blame current line: hash  author  when  summary"),
        category: "Git",
        menu: Some(8),
        description: Some("Show git blame for the current line"),
    },
    ActionSpec {
        id: "toggle_diff_staged",
        palette: Some("Cycle diff source (staged/unstaged)"),
        help: Some("cycle diff source: all (vs HEAD) -> staged -> unstaged"),
        category: "Git",
        menu: Some(9),
        description: Some("Toggle between staged and unstaged diff view"),
    },
    ActionSpec {
        id: "command_palette",
        palette: None,
        help: Some("open command palette (all commands + keys)"),
        category: "General",
        menu: None,
        description: None,
    },
    ActionSpec {
        id: "toggle_watch",
        palette: Some("Toggle auto watch (reload on file change)"),
        help: Some("Toggle auto watch (reload on file change)"),
        category: "View",
        menu: Some(13),
        description: Some("Auto-reload the file tree on filesystem changes"),
    },
    ActionSpec {
        id: "goto_line",
        palette: Some("Go to line"),
        help: Some("Go to line"),
        category: "Navigate",
        menu: Some(4),
        description: Some("Jump to a specific line number"),
    },
    ActionSpec {
        id: "symbol_outline",
        palette: Some("Show symbol outline"),
        help: Some("show the current file's symbol outline"),
        category: "Navigate",
        menu: Some(0),
        description: Some("Filter and jump to symbols in the current file"),
    },
    ActionSpec {
        id: "diagnostics_picker",
        palette: Some("Show diagnostics"),
        help: Some("show diagnostics for the current file"),
        category: "Navigate",
        menu: Some(0),
        description: Some("Filter diagnostics and jump to a reported line"),
    },
    ActionSpec {
        id: "compare_against",
        palette: Some("Compare against a revision"),
        help: None,
        category: "Git",
        menu: Some(10),
        description: Some("Compare the current file against a git revision"),
    },
    ActionSpec {
        id: "toggle_file_revision",
        palette: Some("Toggle file at revision (diff ↔ snapshot)"),
        help: Some("toggle between diff and file at revision (in a revision diff)"),
        category: "Git",
        menu: Some(11),
        description: Some("Switch between revision diff and file content at that commit"),
    },
    ActionSpec {
        id: "blame_open_commit",
        palette: Some("Blame: open file at commit"),
        help: Some("open file at the commit shown on the active blame line"),
        category: "Git",
        menu: Some(12),
        description: Some("View the file content at the commit under the blame cursor"),
    },
    // -- Palette/menu-only actions: no keymap binding --------------------
    ActionSpec {
        id: "open_config_in_editor",
        palette: Some("Open config in editor"),
        help: None,
        category: "General",
        menu: Some(7),
        description: Some("Open the mantis configuration file in your editor"),
    },
    ActionSpec {
        id: "show_about",
        palette: Some("About mantis"),
        help: None,
        category: "General",
        menu: Some(8),
        description: Some("Show version, credits, and release notes"),
    },
    ActionSpec {
        id: "fold_all",
        palette: Some("Fold all"),
        help: None,
        category: "View",
        menu: Some(14),
        description: Some("Fold all foldable regions in the current file"),
    },
    ActionSpec {
        id: "unfold_all",
        palette: Some("Unfold all"),
        help: None,
        category: "View",
        menu: Some(15),
        description: Some("Unfold all folded regions in the current file"),
    },
];

/// The preconditions required for an action to be applicable.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Applicability {
    /// Always applicable.
    Always,
    /// Requires an open file.
    OpenFile,
    /// Requires an open JSON file.
    JsonFile,
    /// Requires an open CSV/TSV file.
    CsvFile,
    /// Requires being in a git repository.
    GitRepo,
    /// Requires being in a git repository and having an open file.
    GitRepoAndFile,
    /// Requires being in a git repository and a text cursor (not in diff).
    GitRepoAndNoDiff,
    /// Requires being in a git repository and a diff view.
    GitRepoAndDiffView,
    /// Requires being in a diff view.
    DiffView,
    /// Requires the current file to have fold regions.
    FoldRegions,
    /// Requires the current file to be rendered by a plugin (active plugin content).
    PluginContentActive,
    /// Requires being in git mode.
    GitMode,
    /// Requires a current file with symbols from its language provider.
    Symbols,
    /// Requires a current file with diagnostics from its language provider.
    Diagnostics,
}

impl ActionSpec {
    /// Returns the applicability precondition for this action.
    pub fn applicability(&self) -> Applicability {
        match self.id {
            "toggle_pretty_json" => Applicability::JsonFile,
            "toggle_table_view" => Applicability::CsvFile,
            "blame_line" | "toggle_blame" => Applicability::GitRepoAndNoDiff,
            "file_history" => Applicability::GitRepoAndFile,
            "repo_commit_log" => Applicability::GitRepo,
            "git_mode_toggle" => Applicability::GitRepo,
            "compare_against" => Applicability::GitRepo,
            "toggle_diff_staged" => Applicability::GitRepoAndDiffView,
            "toggle_diff_side_by_side" | "diff_hunk_next" | "diff_hunk_prev" => {
                Applicability::DiffView
            }
            "toggle_file_revision" => Applicability::GitRepoAndDiffView,
            "blame_open_commit" => Applicability::GitRepoAndNoDiff,
            "fold_toggle" | "fold_all" | "unfold_all" => Applicability::FoldRegions,
            "toggle_raw_markdown" => Applicability::PluginContentActive,
            "open_in_editor" | "open_external" | "copy_path" | "copy_relative_path"
            | "copy_line" | "copy_file" | "goto_line" => Applicability::OpenFile,
            "symbol_outline" => Applicability::Symbols,
            "diagnostics_picker" => Applicability::Diagnostics,
            "git_mode_flat_toggle" => Applicability::GitMode,
            _ => Applicability::Always,
        }
    }
}

#[cfg(test)]
#[path = "actions_test.rs"]
mod tests;
