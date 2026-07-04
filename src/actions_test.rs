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
    "fold_toggle",
    "toggle_watch",
    "recent_files",
    "copy_path",
    "copy_relative_path",
    "plugin_picker",
    "goto_line",
    "tree_up_dir",
];

/// Action ids `app::key_handlers::editor::dispatch_command` has a real match
/// arm for (not falling through to the `_ => {}` no-op). Every
/// `ACTIONS` entry with `palette: Some(_)` must appear here, or selecting it
/// from the Ctrl-P palette would silently do nothing.
const DISPATCH_HANDLED_IDS: &[&str] = &[
    "help",
    "toggle_hidden",
    "search_files",
    "search_content",
    "reload",
    "file_history",
    "theme_picker",
    "plugin_picker",
    "git_mode_toggle",
    "git_mode_flat_toggle",
    "toggle_wrap",
    "toggle_line_numbers",
    "toggle_pretty_json",
    "toggle_blame",
    "toggle_diff_side_by_side",
    "toggle_diff_staged",
    "diff_hunk_next",
    "diff_hunk_prev",
    "open_in_editor",
    "open_config_in_editor",
    "show_about",
    "fold_all",
    "unfold_all",
    "fold_toggle",
    "blame_line",
    "copy_path",
    "copy_relative_path",
    "tree_collapse_all",
    "tree_expand_all",
    "tree_up_dir",
    "recent_files",
    "toggle_watch",
    "goto_line",
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
    "find_files",
    "switch_panel",
    "quit",
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

#[test]
fn every_palette_action_id_is_dispatch_handled() {
    for action in ACTIONS.iter().filter(|a| a.palette.is_some()) {
        assert!(
            DISPATCH_HANDLED_IDS.contains(&action.id),
            "ACTIONS entry '{}' has palette: Some(_) but dispatch_command has \
             no match arm for it - selecting it from Ctrl-P would do nothing",
            action.id,
        );
    }
}

#[test]
fn dispatch_handled_ids_are_all_real_actions() {
    // Guards DISPATCH_HANDLED_IDS against typos / stale entries after a
    // rename in ACTIONS.
    for &id in DISPATCH_HANDLED_IDS {
        assert!(
            ACTIONS.iter().any(|a| a.id == id),
            "'{id}' in DISPATCH_HANDLED_IDS is not a known ACTIONS id",
        );
    }
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
    // diff_hunk_next/prev, toggle_blame.
    for id in [
        "recent_files",
        "toggle_diff_staged",
        "diff_hunk_next",
        "diff_hunk_prev",
        "toggle_blame",
    ] {
        let action = ACTIONS.iter().find(|a| a.id == id);
        assert!(action.is_some(), "'{id}' missing from ACTIONS entirely");
        assert!(
            action.unwrap().palette.is_some(),
            "'{id}' must have a palette entry (was reported missing in #495)",
        );
    }
}
