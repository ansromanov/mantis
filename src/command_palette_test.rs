use super::*;

#[test]
fn command_palette_starts_with_all_commands() {
    let p = CommandPalette::default();
    assert_eq!(p.results_len(), COMMANDS.len());
    assert_eq!(p.selected, 0);
}

#[test]
fn command_palette_push_filters() {
    let mut p = CommandPalette::default();
    p.push('w');
    assert!(p.results_len() < COMMANDS.len());
    let cmd = p.selected_command().unwrap();
    assert!(cmd.name.to_lowercase().contains('w'));
}

#[test]
fn command_palette_pop_restores() {
    let mut p = CommandPalette::default();
    p.push('z');
    let after_push = p.results_len();
    p.pop();
    assert_eq!(p.results_len(), COMMANDS.len());
    assert!(after_push < COMMANDS.len());
}

#[test]
fn command_palette_selected_command() {
    let p = CommandPalette::default();
    let first = p.selected_command();
    assert!(first.is_some());
    assert_eq!(first.unwrap().name, COMMANDS[0].name);
}

#[test]
fn command_palette_selected_index_returns_none_when_empty() {
    let mut p = CommandPalette::default();
    for c in "zzzzzzzzzzzzz".chars() {
        p.push(c);
    }
    assert_eq!(p.results_len(), 0);
    assert!(p.selected_index().is_none());
    assert!(p.selected_command().is_none());
}

#[test]
fn command_palette_results_len() {
    let p = CommandPalette::default();
    assert_eq!(p.results_len(), COMMANDS.len());
}

#[test]
fn command_palette_filters_by_name() {
    let mut p = CommandPalette::default();
    for c in "markdown".chars() {
        p.push(c);
    }
    assert!(p.selected_command().unwrap().name.contains("markdown"));
}

#[test]
fn command_palette_filters_by_keybinding() {
    let mut p = CommandPalette::default();
    for c in "ctrl+g".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "toggle_git_mode");
}

#[test]
fn command_palette_filters_blame_line() {
    let mut p = CommandPalette::default();
    for c in "Blame active".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "blame_line");
}

#[test]
fn go_to_line_command_is_registered() {
    let found = COMMANDS.iter().any(|c| c.action_id == "go_to_line");
    assert!(found, "go_to_line command must be in COMMANDS");
}

#[test]
fn go_to_line_command_is_searchable_by_name() {
    let mut p = CommandPalette::default();
    for c in "Go to line".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "go_to_line");
}

#[test]
fn tree_up_dir_command_is_registered() {
    assert!(
        COMMANDS.iter().any(|c| c.action_id == "tree_up_dir"),
        "tree_up_dir must be in COMMANDS"
    );
}

#[test]
fn tree_up_dir_command_is_searchable_by_name() {
    let mut p = CommandPalette::default();
    for c in "Go up one directory".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "tree_up_dir");
}

// -- ranked_base_order tests -------------------------------------------------

#[test]
fn ranked_base_order_pins_recent_and_frequent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("toggle_help");
    usage.record("toggle_help");
    usage.record("reload");
    usage.record("toggle_hidden");

    let (order, pinned) = ranked_base_order(&usage, true, 2);
    // After record: toggle_help(2), reload(1), toggle_hidden(1, last-used)
    // Pin recent (toggle_hidden idx 1), then top 2 (toggle_help idx 0, reload idx 4)
    assert_eq!(pinned, 3);
    assert_eq!(order[0], 1); // toggle_hidden (most recent)
    assert_eq!(order[1], 0); // toggle_help (most used)
    assert_eq!(order[2], 4); // reload (second most used)
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_no_recent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("toggle_help");
    usage.record("reload");

    let (order, pinned) = ranked_base_order(&usage, false, 2);
    // Both count 1, alphabetical tie-break: reload < toggle_help
    // reload idx 4, toggle_help idx 0
    assert_eq!(pinned, 2);
    assert_eq!(order[0], 4); // reload (alphabetical tie-break)
    assert_eq!(order[1], 0); // toggle_help
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_no_frequent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("toggle_help");

    let (order, pinned) = ranked_base_order(&usage, true, 0);
    // Only recent pinned
    assert_eq!(pinned, 1);
    assert_eq!(order[0], 0); // toggle_help (both recent and top-0, but recent wins)
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_unknown_action_id_ignored() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("nonexistent_command");
    usage.record("also_missing");

    let (order, pinned) = ranked_base_order(&usage, true, 3);
    // Both unknown, so nothing pinned
    assert_eq!(pinned, 0);
    assert_eq!(order[0], 0); // first command in source order
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_empty_usage() {
    let usage = crate::command_usage::UsageStats::default();
    let (order, pinned) = ranked_base_order(&usage, true, 3);
    assert_eq!(pinned, 0);
    // Natural source order
    for (i, &idx) in order.iter().enumerate() {
        assert_eq!(idx, i);
    }
    assert!(order.len() == COMMANDS.len());
}

#[test]
fn ranked_base_order_recent_and_frequent_overlap() {
    let mut usage = crate::command_usage::UsageStats::default();
    // Most-recent is also the most-used; should not duplicate
    usage.record("toggle_help");
    usage.record("toggle_help");
    usage.record("reload");

    let (order, pinned) = ranked_base_order(&usage, true, 3);
    // pinned: recent (reload) + top 3 (toggle_help, reload) = 2 unique
    // order: reload(4) first, then toggle_help(0), then rest
    assert_eq!(pinned, 2);
    assert_eq!(order[0], 4); // reload (recent, last-used)
    assert_eq!(order[1], 0); // toggle_help (most-used)
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_typing_query_still_fuzzy() {
    // ranked_base_order only affects the empty-query ordering.
    // After typing a query, push/pop/refilter should use fuzzy matching.
    let usage = crate::command_usage::UsageStats::default();
    let (base, _) = ranked_base_order(&usage, true, 3);
    let mut p = CommandPalette::new(&Keymap::default(), base, 0);
    assert_eq!(p.results_len(), COMMANDS.len());
    // Type a query — results should be fuzzy-filtered
    for c in "git".chars() {
        p.push(c);
    }
    assert!(p.results_len() < COMMANDS.len());
    assert_eq!(p.selected_command().unwrap().action_id, "toggle_git_mode");
}

fn is_permutation(order: &[usize]) -> bool {
    let mut sorted = order.to_vec();
    sorted.sort_unstable();
    sorted.iter().copied().eq(0..COMMANDS.len())
}

#[test]
fn command_palette_list_picker_impl_delegates() {
    use crate::list_picker::ListPicker;
    let usage = crate::command_usage::UsageStats::default();
    let (base, _) = ranked_base_order(&usage, true, 0);
    let mut p = CommandPalette::new(&Keymap::default(), base, 0);
    assert!(ListPicker::query_is_empty(&p));
    assert_eq!(ListPicker::selected(&p), 0);
    ListPicker::set_selected(&mut p, 1);
    assert_eq!(p.selected, 1);
    ListPicker::query_push(&mut p, 'g');
    assert!(!ListPicker::query_is_empty(&p));
    let len_after_push = ListPicker::results_len(&p);
    assert!(
        len_after_push < COMMANDS.len(),
        "push should filter results"
    );
    ListPicker::query_pop(&mut p);
    assert!(ListPicker::query_is_empty(&p));
    assert_eq!(ListPicker::results_len(&p), COMMANDS.len());
}
