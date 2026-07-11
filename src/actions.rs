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
    /// Optional group label for the command palette (e.g. `"Git"`, `"View"`).
    /// When set, the palette prefixes entries with `"{category}: "`, making the
    /// palette browsable by domain via fuzzy matching.
    pub category: Option<&'static str>,
    /// Optional one-line description shown dim after the palette entry name.
    /// Makes the palette self-documenting.
    pub description: Option<&'static str>,
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
        category: Some("General"),
        description: Some("Toggle the keybinding help overlay"),
    },
    ActionSpec {
        id: "bug_report",
        palette: Some("Report a bug (save diagnostics locally)"),
        help: None,
        category: Some("General"),
        description: Some("Save diagnostics for a bug report"),
    },
    ActionSpec {
        id: "switch_panel",
        palette: None,
        help: Some(("Global", "switch panel")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "quit",
        palette: Some("Quit"),
        help: Some(("Global", "quit")),
        category: Some("General"),
        description: Some("Exit the application"),
    },
    ActionSpec {
        id: "toggle_hidden",
        palette: Some("Toggle hidden files"),
        help: Some(("Global", "toggle hidden files")),
        category: Some("General"),
        description: Some("Show or hide dotfiles in the tree"),
    },
    ActionSpec {
        id: "toggle_telemetry",
        palette: Some("Toggle telemetry"),
        help: None,
        category: Some("General"),
        description: Some("Enable or disable local usage telemetry"),
    },
    ActionSpec {
        id: "theme_picker",
        palette: Some("Open theme picker"),
        help: Some(("Global", "pick a theme")),
        category: Some("View"),
        description: Some("Switch the color theme"),
    },
    ActionSpec {
        id: "plugin_picker",
        palette: Some("Open plugin manager"),
        help: Some(("Global", "plugin manager")),
        category: Some("View"),
        description: Some("Manage installed plugins"),
    },
    ActionSpec {
        id: "git_mode_toggle",
        palette: Some("Toggle git mode"),
        help: Some(("Global", "toggle git mode (changed files only + diffs)")),
        category: Some("Git"),
        description: Some("Filter tree to changed files and show diffs"),
    },
    ActionSpec {
        id: "open_in_editor",
        palette: Some("Open in editor"),
        help: Some(("Global", "open file in $EDITOR")),
        category: Some("General"),
        description: Some("Open the selected file in your editor"),
    },
    ActionSpec {
        id: "open_external",
        palette: Some("Open with default app"),
        help: Some(("Global", "open file with system default app")),
        category: Some("General"),
        description: Some("Open the selected file with the system default app"),
    },
    ActionSpec {
        id: "copy_path",
        palette: Some("Copy absolute path"),
        help: Some(("Global", "copy absolute path to clipboard")),
        category: Some("Copy"),
        description: Some("Copy the absolute file path to the clipboard"),
    },
    ActionSpec {
        id: "copy_relative_path",
        palette: Some("Copy relative path"),
        help: Some(("Global", "copy path relative to tree root to clipboard")),
        category: Some("Copy"),
        description: Some("Copy the relative file path to the clipboard"),
    },
    ActionSpec {
        id: "copy_line",
        palette: Some("Copy line or selection"),
        help: Some((
            "Content panel",
            "copy current line (or selection if any) to clipboard",
        )),
        category: Some("Copy"),
        description: Some("Copy the current line or visual selection"),
    },
    ActionSpec {
        id: "copy_file",
        palette: Some("Copy entire file"),
        help: Some(("Content panel", "copy entire file content to clipboard")),
        category: Some("Copy"),
        description: Some("Copy the entire file content to the clipboard"),
    },
    ActionSpec {
        id: "recent_files",
        palette: Some("Recent files"),
        help: Some(("Global", "recent files picker")),
        category: Some("Navigate"),
        description: Some("Browse and open recently opened files"),
    },
    ActionSpec {
        id: "git_mode_flat_toggle",
        palette: Some("Toggle git flat mode"),
        help: Some(("Global", "toggle git flat/tree view (in git mode)")),
        category: Some("Git"),
        description: Some("Switch between flat and tree view in git mode"),
    },
    // -- Tree panel ---------------------------------------------------------
    ActionSpec {
        id: "nav_up",
        palette: None,
        help: Some(("Tree panel", "move up")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "nav_down",
        palette: None,
        help: Some(("Tree panel", "move down")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "tree_expand",
        palette: None,
        help: Some(("Tree panel", "expand dir / open file")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "tree_collapse",
        palette: None,
        help: Some(("Tree panel", "collapse dir")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "tree_up_dir",
        palette: Some("Go up one directory"),
        help: Some(("Tree panel", "go up one directory")),
        category: Some("Tree"),
        description: Some("Navigate to the parent directory"),
    },
    ActionSpec {
        id: "tree_collapse_all",
        palette: Some("Collapse all directories"),
        help: Some(("Tree panel", "collapse all directories")),
        category: Some("Tree"),
        description: Some("Collapse every expanded directory"),
    },
    ActionSpec {
        id: "tree_expand_all",
        palette: Some("Expand all directories"),
        help: Some(("Tree panel", "expand all directories")),
        category: Some("Tree"),
        description: Some("Expand every directory in the tree"),
    },
    ActionSpec {
        id: "tree_width_grow",
        palette: Some("Grow tree pane width"),
        help: Some(("Tree panel", "increase tree pane width")),
        category: Some("View"),
        description: Some("Increase the tree pane width percentage"),
    },
    ActionSpec {
        id: "tree_width_shrink",
        palette: Some("Shrink tree pane width"),
        help: Some(("Tree panel", "decrease tree pane width")),
        category: Some("View"),
        description: Some("Decrease the tree pane width percentage"),
    },
    ActionSpec {
        id: "find_files",
        palette: Some("Find files"),
        help: Some(("Tree panel", "global fuzzy file-name picker")),
        category: Some("Tree"),
        description: Some("Fuzzy-find files by name across the whole tree"),
    },
    ActionSpec {
        id: "search_files",
        palette: Some("Open file search"),
        help: Some(("Tree panel", "tree filter / in-file search")),
        category: Some("Tree"),
        description: Some("Filter the tree by file name"),
    },
    ActionSpec {
        id: "search_content",
        palette: Some("Open content search"),
        help: Some(("Tree panel", "fuzzy content search")),
        category: Some("Tree"),
        description: Some("Search file contents across the whole tree"),
    },
    ActionSpec {
        id: "reload",
        palette: Some("Reload"),
        help: Some(("Tree panel", "reload tree")),
        category: Some("Tree"),
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
        help: Some(("Content panel", "page scroll")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_page_down",
        palette: None,
        help: Some(("Content panel", "page scroll")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_left",
        palette: None,
        help: Some(("Content panel", "horizontal scroll")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_right",
        palette: None,
        help: Some(("Content panel", "horizontal scroll")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_reset_col",
        palette: None,
        help: Some(("Content panel", "reset horizontal scroll")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_top",
        palette: None,
        help: Some(("Content panel", "go to top")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "content_bottom",
        palette: None,
        help: Some(("Content panel", "go to bottom")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "toggle_wrap",
        palette: Some("Toggle word wrap"),
        help: Some(("Content panel", "toggle word wrap")),
        category: Some("View"),
        description: Some("Toggle word wrapping in the content panel"),
    },
    ActionSpec {
        id: "toggle_line_numbers",
        palette: Some("Toggle line numbers"),
        help: Some(("Content panel", "toggle line numbers")),
        category: Some("View"),
        description: Some("Toggle the line number gutter"),
    },
    ActionSpec {
        id: "toggle_blame",
        palette: Some("Toggle blame"),
        help: Some(("Content panel", "toggle git blame gutter")),
        category: Some("View"),
        description: Some("Toggle the git blame annotation gutter"),
    },
    ActionSpec {
        id: "file_history",
        palette: Some("Open file history"),
        help: Some(("Content panel", "git history of current file")),
        category: Some("Git"),
        description: Some("Browse the git log for the current file"),
    },
    ActionSpec {
        id: "repo_commit_log",
        palette: Some("Browse repository commits"),
        help: Some(("Global", "browse repository-wide commit log")),
        category: Some("Git"),
        description: Some("Browse all commits in the repository"),
    },
    ActionSpec {
        id: "toggle_diff_side_by_side",
        palette: Some("Toggle side-by-side diff"),
        help: Some(("Content panel", "toggle side-by-side diff (in a diff)")),
        category: Some("Diff"),
        description: Some("Switch between unified and side-by-side diff view"),
    },
    ActionSpec {
        id: "diff_hunk_next",
        palette: Some("Next diff hunk"),
        help: Some(("Content panel", "next / previous hunk (in a diff)")),
        category: Some("Diff"),
        description: Some("Jump to the next diff hunk"),
    },
    ActionSpec {
        id: "diff_hunk_prev",
        palette: Some("Previous diff hunk"),
        help: Some(("Content panel", "next / previous hunk (in a diff)")),
        category: Some("Diff"),
        description: Some("Jump to the previous diff hunk"),
    },
    ActionSpec {
        id: "fold_toggle",
        palette: Some("Toggle fold at cursor"),
        help: Some(("Content panel", "toggle fold at cursor")),
        category: Some("Fold"),
        description: Some("Fold or unfold the section at the cursor"),
    },
    ActionSpec {
        id: "toggle_raw_markdown",
        palette: Some("Toggle markdown render (markdown plugin)"),
        help: Some((
            "Content panel",
            "toggle markdown render (md files, markdown plugin)",
        )),
        category: Some("View"),
        description: Some("Toggle between rendered and raw markdown"),
    },
    // -- Bound actions with no keymap-section help row -------------------
    // (either purely a Git-section row, sourced from these same ids with
    // git-specific phrasing, or currently undocumented in the help overlay —
    // unchanged from before this registry existed.)
    ActionSpec {
        id: "toggle_pretty_json",
        palette: Some("Toggle JSON pretty-print"),
        help: None,
        category: Some("View"),
        description: Some("Toggle JSON pretty-printing in the content panel"),
    },
    ActionSpec {
        id: "blame_line",
        palette: Some("Blame active line"),
        help: None,
        category: Some("Git"),
        description: Some("Show git blame for the current line"),
    },
    ActionSpec {
        id: "toggle_diff_staged",
        palette: Some("Cycle diff source (staged/unstaged)"),
        help: None,
        category: Some("Git"),
        description: Some("Toggle between staged and unstaged diff view"),
    },
    ActionSpec {
        id: "command_palette",
        palette: None,
        help: Some(("Global", "open command palette (all commands + keys)")),
        category: None,
        description: None,
    },
    ActionSpec {
        id: "toggle_watch",
        palette: Some("Toggle auto watch (reload on file change)"),
        help: None,
        category: Some("View"),
        description: Some("Auto-reload the file tree on filesystem changes"),
    },
    ActionSpec {
        id: "goto_line",
        palette: Some("Go to line"),
        help: None,
        category: Some("Navigate"),
        description: Some("Jump to a specific line number"),
    },
    ActionSpec {
        id: "compare_against",
        palette: Some("Compare against a revision"),
        help: None,
        category: Some("Git"),
        description: Some("Compare the current file against a git revision"),
    },
    ActionSpec {
        id: "toggle_file_revision",
        palette: Some("Toggle file at revision (diff ↔ snapshot)"),
        help: Some((
            "Content panel",
            "toggle between diff and file at revision (in a revision diff)",
        )),
        category: Some("Git"),
        description: Some("Switch between revision diff and file content at that commit"),
    },
    ActionSpec {
        id: "blame_open_commit",
        palette: Some("Blame: open file at commit"),
        help: Some((
            "Blame annotation",
            "open file at the commit shown on the active blame line",
        )),
        category: Some("Git"),
        description: Some("View the file content at the commit under the blame cursor"),
    },
    // -- Palette/menu-only actions: no keymap binding --------------------
    ActionSpec {
        id: "open_config_in_editor",
        palette: Some("Open config in editor"),
        help: None,
        category: Some("General"),
        description: Some("Open the mantis configuration file in your editor"),
    },
    ActionSpec {
        id: "show_about",
        palette: Some("About mantis"),
        help: None,
        category: Some("General"),
        description: Some("Show version, credits, and release notes"),
    },
    ActionSpec {
        id: "fold_all",
        palette: Some("Fold all"),
        help: None,
        category: Some("Fold"),
        description: Some("Fold all foldable regions in the current file"),
    },
    ActionSpec {
        id: "unfold_all",
        palette: Some("Unfold all"),
        help: None,
        category: Some("Fold"),
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
}

impl ActionSpec {
    /// Returns the applicability precondition for this action.
    pub fn applicability(&self) -> Applicability {
        match self.id {
            "toggle_pretty_json" => Applicability::JsonFile,
            "blame_line" | "toggle_blame" => Applicability::GitRepoAndNoDiff,
            "file_history" => Applicability::GitRepoAndFile,
            "repo_commit_log" => Applicability::GitRepo,
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
            "git_mode_flat_toggle" => Applicability::GitMode,
            _ => Applicability::Always,
        }
    }
}

#[cfg(test)]
#[path = "actions_test.rs"]
mod tests;
