use super::*;

/// Every `Keymap` field that represents a real bound action, by its canonical
/// action id (matching the field name, or the id `Keymap::bindings_for_action`
/// maps to that field - see `config::keymap`). Kept in sync by hand since Rust
/// has no field reflection; a compile error in `Keymap` construction elsewhere
/// in the test suite would catch a renamed/removed field long before this list
/// goes stale silently.
const KEYMAP_FIELD_ACTION_IDS: &[&str] = &[
    "quit",
    "help",
    "toggle_hidden",
    "search_files",
    "find_files",
    "search_content",
    "reload",
    "switch_panel",
    "file_history",
    "repo_commit_log",
    "theme_picker",
    "nav_up",
    "nav_down",
    "tree_expand",
    "tree_collapse",
    "tree_collapse_all",
    "tree_expand_all",
    "content_left",
    "content_right",
    "content_top",
    "content_bottom",
    "content_page_up",
    "content_page_down",
    "content_reset_col",
    "toggle_wrap",
    "toggle_line_numbers",
    "toggle_pretty_json",
    "toggle_raw_markdown",
    "toggle_blame",
    "blame_line",
    "toggle_diff_side_by_side",
    "toggle_diff_staged",
    "diff_hunk_next",
    "diff_hunk_prev",
    "git_mode_toggle",
    "git_mode_flat_toggle",
    "command_palette",
    "open_in_editor",
    "open_external",
    "fold_toggle",
    "toggle_watch",
    "recent_files",
    "copy_path",
    "copy_relative_path",
    "copy_line",
    "copy_file",
    "plugin_picker",
    "goto_line",
    "tree_up_dir",
    "tree_width_grow",
    "tree_width_shrink",
    "toggle_file_revision",
    "blame_open_commit",
];

/// Pure-navigation actions that are inherently keymap-only: they should never
/// need a palette entry, and their help-overlay coverage (if any) is already
/// captured by their `ACTIONS` entry's `.help` field. Listed here only so the
/// parity test below can assert "every keymap field has an ACTIONS entry"
/// without also demanding they carry a palette name.
const NAV_ONLY_ALLOWLIST: &[&str] = &[
    "nav_up",
    "nav_down",
    "tree_expand",
    "tree_collapse",
    "content_left",
    "content_right",
    "content_top",
    "content_bottom",
    "content_page_up",
    "content_page_down",
    "content_reset_col",
    "switch_panel",
    "command_palette",
];

#[test]
fn every_keymap_field_has_an_actions_entry() {
    for &id in KEYMAP_FIELD_ACTION_IDS {
        assert!(
            ACTIONS.iter().any(|a| a.id == id),
            "keymap field/action '{id}' has no ACTIONS entry - the action \
             registry has drifted from the keymap again",
        );
    }
}

#[test]
fn nav_only_allowlist_ids_are_real_keymap_fields() {
    // Guards the allowlist itself against typos / stale entries.
    for &id in NAV_ONLY_ALLOWLIST {
        assert!(
            KEYMAP_FIELD_ACTION_IDS.contains(&id),
            "'{id}' in NAV_ONLY_ALLOWLIST is not a known keymap field id",
        );
    }
}

#[test]
fn nav_only_allowlist_entries_have_no_palette_entry() {
    // Pure-navigation actions should not clutter the command palette.
    for &id in NAV_ONLY_ALLOWLIST {
        if let Some(action) = ACTIONS.iter().find(|a| a.id == id) {
            assert!(
                action.palette.is_none(),
                "'{id}' is in NAV_ONLY_ALLOWLIST but has a palette entry \
                 ({:?}) - remove it from the allowlist or drop its palette name",
                action.palette,
            );
        }
    }
}

#[test]
fn every_bound_non_nav_action_has_a_palette_entry() {
    // Every keymap field not covered by the pure-navigation allowlist must be
    // reachable from the command palette (this is the actual bug the issue
    // reported: keys bound but not discoverable via Ctrl-P).
    for &id in KEYMAP_FIELD_ACTION_IDS {
        if NAV_ONLY_ALLOWLIST.contains(&id) {
            continue;
        }
        let action = ACTIONS
            .iter()
            .find(|a| a.id == id)
            .unwrap_or_else(|| panic!("'{id}' missing from ACTIONS"));
        assert!(
            action.palette.is_some(),
            "'{id}' is bound but has no palette entry - it would be \
             undiscoverable via Ctrl-P",
        );
    }
}

/// Every `ACTIONS` entry with `palette: Some(_)` must be reachable through
/// `App::dispatch_command`'s real match arms - not a hand-maintained list of
/// ids that could drift from the match itself. `dispatch_command` returns
/// `true` iff `action_id` matched a literal arm (regardless of that arm's own
/// state-dependent guard, e.g. `is_diff`), so removing a match arm here fails
/// this test immediately instead of leaving a stale allowlist green.
#[test]
fn every_palette_action_id_is_dispatch_handled() {
    use crate::app::App;
    use crate::command_palette::CommandPalette;
    use crate::config::Config;

    // Dispatch arms run for real (bug_report writes a file, command usage
    // saves), so point the state dir at an isolated temp directory.
    let _guard = crate::session::STATE_DIR_ENV_LOCK.lock().unwrap();
    let state = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", state.path());

    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(dir.path().to_path_buf(), Config::default(), None, None).unwrap();

    for action in ACTIONS.iter().filter(|a| a.palette.is_some()) {
        let idx = crate::command_palette::COMMANDS
            .iter()
            .position(|c| c.action_id == action.id)
            .unwrap_or_else(|| panic!("'{}' missing from COMMANDS", action.id));
        app.command_palette = Some(CommandPalette::default());
        if let Some(p) = &mut app.command_palette {
            p.filtered = vec![idx];
            p.selected = 0;
        }
        assert!(
            app.dispatch_command(),
            "ACTIONS entry '{}' has palette: Some(_) but dispatch_command has \
             no match arm for it - selecting it from Ctrl-P would do nothing",
            action.id,
        );
    }
    drop(app);
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn no_duplicate_action_ids() {
    for (i, a) in ACTIONS.iter().enumerate() {
        for b in &ACTIONS[i + 1..] {
            assert_ne!(
                a.id, b.id,
                "duplicate ACTIONS id '{}' - exactly one entry per action",
                a.id
            );
        }
    }
}

#[test]
fn missing_palette_entries_from_issue_495_are_present() {
    // Regression guard for the specific actions the issue called out as
    // missing from the palette: recent_files, toggle_diff_staged,
    // diff_hunk_next/prev, toggle_blame, quit.
    for id in [
        "recent_files",
        "toggle_diff_staged",
        "diff_hunk_next",
        "diff_hunk_prev",
        "toggle_blame",
        "quit",
    ] {
        let action = ACTIONS.iter().find(|a| a.id == id);
        assert!(action.is_some(), "'{id}' missing from ACTIONS entirely");
        assert!(
            action.unwrap().palette.is_some(),
            "'{id}' must have a palette entry (was reported missing in #495)",
        );
    }
}

#[test]
fn find_files_is_palette_invokable() {
    // With mostly-unbound content-pane toggles (issue #298), the palette is
    // the guaranteed fallback surface; find_files must be reachable there.
    let action = ACTIONS.iter().find(|a| a.id == "find_files").unwrap();
    assert!(
        action.palette.is_some(),
        "find_files must have a palette entry"
    );
}

#[test]
fn command_palette_has_help_section() {
    let action = ACTIONS.iter().find(|a| a.id == "command_palette").unwrap();
    assert!(
        action.help.is_some(),
        "command_palette must have a help section"
    );
}

#[test]
fn toggle_telemetry_action_is_palette_only() {
    let action = ACTIONS
        .iter()
        .find(|a| a.id == "toggle_telemetry")
        .expect("toggle_telemetry must be registered");
    assert!(action.palette.is_some(), "reachable via Ctrl-P");
    assert!(
        !KEYMAP_FIELD_ACTION_IDS.contains(&"toggle_telemetry"),
        "palette-only: no keymap field expected"
    );
}

#[test]
fn bug_report_action_is_palette_only() {
    let action = ACTIONS
        .iter()
        .find(|a| a.id == "bug_report")
        .expect("bug_report must be registered");
    assert!(action.palette.is_some(), "reachable via Ctrl-P");
    assert!(
        !KEYMAP_FIELD_ACTION_IDS.contains(&"bug_report"),
        "palette-only: no keymap field expected"
    );
}

#[test]
fn compare_against_action_is_palette_only() {
    let action = ACTIONS
        .iter()
        .find(|a| a.id == "compare_against")
        .expect("compare_against must be registered");
    assert!(action.palette.is_some(), "reachable via Ctrl-P");
    assert!(
        !KEYMAP_FIELD_ACTION_IDS.contains(&"compare_against"),
        "palette-only: no keymap field expected"
    );
}

// -- category & description tests -------------------------------------------

#[test]
fn every_palette_entry_has_category() {
    for action in ACTIONS.iter().filter(|a| a.palette.is_some()) {
        assert!(
            action.category.is_some(),
            "action '{}' has palette entry but no category",
            action.id,
        );
    }
}

#[test]
fn every_palette_entry_has_description() {
    for action in ACTIONS.iter().filter(|a| a.palette.is_some()) {
        assert!(
            action.description.is_some(),
            "action '{}' has palette entry but no description",
            action.id,
        );
    }
}

#[test]
fn navigation_actions_have_no_category_or_description() {
    for action in ACTIONS.iter().filter(|a| a.palette.is_none()) {
        assert!(
            action.category.is_none(),
            "action '{}' has no palette entry but has a category",
            action.id,
        );
        assert!(
            action.description.is_none(),
            "action '{}' has no palette entry but has a description",
            action.id,
        );
    }
}

// -- applicability tests -----------------------------------------------------

fn applicability_of(id: &str) -> Applicability {
    ACTIONS.iter().find(|a| a.id == id).unwrap().applicability()
}

#[test]
fn applicability_defaults_to_always_for_unlisted_actions() {
    assert_eq!(applicability_of("quit"), Applicability::Always);
    assert_eq!(applicability_of("help"), Applicability::Always);
}

#[test]
fn applicability_maps_each_special_cased_action_to_its_precondition() {
    assert_eq!(
        applicability_of("toggle_pretty_json"),
        Applicability::JsonFile
    );
    assert_eq!(
        applicability_of("blame_line"),
        Applicability::GitRepoAndNoDiff
    );
    assert_eq!(
        applicability_of("toggle_blame"),
        Applicability::GitRepoAndNoDiff
    );
    assert_eq!(
        applicability_of("file_history"),
        Applicability::GitRepoAndFile
    );
    assert_eq!(applicability_of("compare_against"), Applicability::GitRepo);
    assert_eq!(
        applicability_of("toggle_diff_staged"),
        Applicability::GitRepoAndDiffView
    );
    assert_eq!(
        applicability_of("toggle_diff_side_by_side"),
        Applicability::DiffView
    );
    assert_eq!(applicability_of("diff_hunk_next"), Applicability::DiffView);
    assert_eq!(applicability_of("diff_hunk_prev"), Applicability::DiffView);
    assert_eq!(applicability_of("fold_toggle"), Applicability::FoldRegions);
    assert_eq!(applicability_of("fold_all"), Applicability::FoldRegions);
    assert_eq!(applicability_of("unfold_all"), Applicability::FoldRegions);
    assert_eq!(
        applicability_of("toggle_raw_markdown"),
        Applicability::PluginContentActive
    );
    assert_eq!(applicability_of("open_in_editor"), Applicability::OpenFile);
    assert_eq!(applicability_of("open_external"), Applicability::OpenFile);
    assert_eq!(applicability_of("copy_path"), Applicability::OpenFile);
    assert_eq!(
        applicability_of("copy_relative_path"),
        Applicability::OpenFile
    );
    assert_eq!(applicability_of("copy_line"), Applicability::OpenFile);
    assert_eq!(applicability_of("copy_file"), Applicability::OpenFile);
    assert_eq!(applicability_of("goto_line"), Applicability::OpenFile);
    assert_eq!(
        applicability_of("git_mode_flat_toggle"),
        Applicability::GitMode
    );
}

#[test]
fn repo_commit_log_action_requires_git_repo() {
    assert_eq!(applicability_of("repo_commit_log"), Applicability::GitRepo);
    let spec = ACTIONS
        .iter()
        .find(|a| a.id == "repo_commit_log")
        .expect("repo_commit_log must be registered in ACTIONS");
    assert_eq!(spec.palette, Some("Browse repository commits"));
    assert_eq!(spec.category, Some("Git"));
}
