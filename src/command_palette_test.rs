use super::*;

#[test]
fn commands_have_categories() {
    let git_idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "git_mode_toggle")
        .expect("git_mode_toggle must be in COMMANDS");
    assert_eq!(COMMANDS[git_idx].category.as_deref(), Some("Git"));

    let copy_idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "copy_path")
        .expect("copy_path must be in COMMANDS");
    assert_eq!(COMMANDS[copy_idx].category.as_deref(), Some("Copy"));

    let view_idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "toggle_wrap")
        .expect("toggle_wrap must be in COMMANDS");
    assert_eq!(COMMANDS[view_idx].category.as_deref(), Some("View"));

    let tree_idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "find_files")
        .expect("find_files must be in COMMANDS");
    assert_eq!(COMMANDS[tree_idx].category.as_deref(), Some("Tree"));
}

#[test]
fn commands_have_descriptions() {
    let help_idx = COMMANDS
        .iter()
        .position(|c| c.action_id == "help")
        .expect("help must be in COMMANDS");
    assert!(
        COMMANDS[help_idx].description.is_some(),
        "help command should have a description"
    );
    assert!(
        !COMMANDS[help_idx]
            .description
            .as_deref()
            .unwrap()
            .is_empty(),
        "description should not be empty"
    );
}

#[test]
fn every_palette_entry_has_category() {
    for cmd in COMMANDS.iter() {
        assert!(
            cmd.category.is_some(),
            "command '{}' ({}) is missing a category",
            cmd.name,
            cmd.action_id,
        );
    }
}

#[test]
fn every_palette_entry_has_description() {
    for cmd in COMMANDS.iter() {
        assert!(
            cmd.description.is_some(),
            "command '{}' ({}) is missing a description",
            cmd.name,
            cmd.action_id,
        );
    }
}

fn index_of(id: &str) -> usize {
    COMMANDS
        .iter()
        .position(|c| c.action_id == id)
        .unwrap_or_else(|| panic!("'{id}' not found in COMMANDS"))
}

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
    for c in "json".chars() {
        p.push(c);
    }
    assert!(p.selected_command().unwrap().name.contains("JSON"));
}

#[test]
fn command_palette_filters_by_keybinding() {
    let mut p = CommandPalette::default();
    for c in "Ctrl+D".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "git_mode_toggle");
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
    let found = COMMANDS.iter().any(|c| c.action_id == "goto_line");
    assert!(found, "goto_line command must be in COMMANDS");
}

#[test]
fn go_to_line_command_is_searchable_by_name() {
    let mut p = CommandPalette::default();
    for c in "Go to line".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "goto_line");
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

// -- newly-added palette entries (issue #495) --------------------------------

#[test]
fn recent_files_command_is_registered_and_searchable() {
    assert!(COMMANDS.iter().any(|c| c.action_id == "recent_files"));
    let mut p = CommandPalette::default();
    for c in "Recent files".chars() {
        p.push(c);
    }
    assert_eq!(p.selected_command().unwrap().action_id, "recent_files");
}

#[test]
fn toggle_diff_staged_command_is_registered() {
    assert!(COMMANDS.iter().any(|c| c.action_id == "toggle_diff_staged"));
}

#[test]
fn diff_hunk_next_and_prev_commands_are_registered() {
    assert!(COMMANDS.iter().any(|c| c.action_id == "diff_hunk_next"));
    assert!(COMMANDS.iter().any(|c| c.action_id == "diff_hunk_prev"));
}

#[test]
fn toggle_blame_command_is_registered_and_searchable() {
    assert!(COMMANDS.iter().any(|c| c.action_id == "toggle_blame"));
    let mut p = CommandPalette::default();
    for c in "Toggle blame".chars() {
        p.push(c);
    }
    assert_eq!(p.selected_command().unwrap().action_id, "toggle_blame");
}

// -- ranked_base_order tests -------------------------------------------------

#[test]
fn ranked_base_order_pins_recent_and_frequent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("help");
    usage.record("help");
    usage.record("reload");
    usage.record("toggle_hidden");

    let (order, pinned) = ranked_base_order(&usage, true, 2);
    // After record: help(2), reload(1), toggle_hidden(1, last-used)
    // Pin recent (toggle_hidden), then top 2 (help most-used, reload second)
    assert_eq!(pinned, 3);
    assert_eq!(order[0], index_of("toggle_hidden")); // most recent
    assert_eq!(order[1], index_of("help")); // most used
    assert_eq!(order[2], index_of("reload")); // second most used
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_no_recent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("help");
    usage.record("reload");

    let (order, pinned) = ranked_base_order(&usage, false, 2);
    // Both count 1, alphabetical tie-break: reload < help by name doesn't
    // apply here (ranked_base_order ties break by ACTIONS/COMMANDS order,
    // not alphabetically) - reload appears later in COMMANDS than help.
    assert_eq!(pinned, 2);
    assert!(order[..2].contains(&index_of("help")));
    assert!(order[..2].contains(&index_of("reload")));
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_no_frequent() {
    let mut usage = crate::command_usage::UsageStats::default();
    usage.record("help");

    let (order, pinned) = ranked_base_order(&usage, true, 0);
    // Only recent pinned
    assert_eq!(pinned, 1);
    assert_eq!(order[0], index_of("help")); // both recent and top-0, recent wins
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
    usage.record("help");
    usage.record("help");
    usage.record("reload");

    let (order, pinned) = ranked_base_order(&usage, true, 3);
    // pinned: recent (reload) + top 2 (help, reload) = 2 unique
    assert_eq!(pinned, 2);
    assert_eq!(order[0], index_of("reload")); // recent, last-used
    assert_eq!(order[1], index_of("help")); // most-used
    assert!(order.len() == COMMANDS.len());
    assert!(is_permutation(&order));
}

#[test]
fn ranked_base_order_typing_query_still_fuzzy() {
    // ranked_base_order only affects the empty-query ordering.
    // After typing a query, push/pop/refilter should use fuzzy matching.
    let usage = crate::command_usage::UsageStats::default();
    let (base, _) = ranked_base_order(&usage, true, 3);
    let mut p = CommandPalette::new(
        &Keymap::default(),
        base,
        0,
        vec![None; COMMANDS.len()],
        Vec::new(),
    );
    assert_eq!(p.results_len(), COMMANDS.len());
    // Type a query — results should be fuzzy-filtered
    for c in "git".chars() {
        p.push(c);
    }
    assert!(p.results_len() < COMMANDS.len());
    assert_eq!(p.selected_command().unwrap().action_id, "git_mode_toggle");
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
    let mut p = CommandPalette::new(
        &Keymap::default(),
        base,
        0,
        vec![None; COMMANDS.len()],
        Vec::new(),
    );
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

#[test]
fn command_palette_match_positions_populated_for_query() {
    let mut p = CommandPalette::default();
    // Empty query -> no match positions
    assert_eq!(p.match_positions.len(), COMMANDS.len());
    assert!(
        p.match_positions.iter().all(|v| v.is_empty()),
        "match positions should be empty when query is empty"
    );

    // Typing a query populates match positions
    for c in "git".chars() {
        p.push(c);
    }
    assert_eq!(p.match_positions.len(), p.filtered.len());
    for (pos, positions) in p.match_positions.iter().enumerate() {
        assert!(
            !positions.is_empty(),
            "entry {} of {} should have match positions for query 'git'",
            pos,
            p.filtered.len(),
        );
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "git_mode_toggle");
}

#[test]
fn command_palette_refilter_excludes_non_matching() {
    let mut p = CommandPalette::default();
    for c in "zzzzzzzzz".chars() {
        p.push(c);
    }
    assert_eq!(p.results_len(), 0);
}

#[test]
fn command_palette_refilter_scores_by_relevance() {
    let mut p = CommandPalette::default();
    // "fold" matches "Fold all" and "Unfold all" and "Toggle fold at cursor"
    for c in "fold".chars() {
        p.push(c);
    }
    let results = p.results_len();
    assert!(results > 0, "expected at least one match for 'fold'");
    let first = p.selected_command().unwrap();
    assert!(
        first.name.to_lowercase().contains("fold"),
        "top result '{}' should be relevant to query 'fold'",
        first.name
    );
}

// -- prefix routing tests ---------------------------------------------------

#[test]
fn palette_route_from_char_prefixes() {
    assert_eq!(PaletteRoute::from_char('>'), Some(PaletteRoute::Commands));
    assert_eq!(PaletteRoute::from_char('/'), Some(PaletteRoute::Files));
    assert_eq!(PaletteRoute::from_char('#'), Some(PaletteRoute::Content));
    assert_eq!(PaletteRoute::from_char(':'), Some(PaletteRoute::GotoLine));
}

#[test]
fn palette_route_from_char_non_prefix_returns_none() {
    assert_eq!(PaletteRoute::from_char('a'), None);
    assert_eq!(PaletteRoute::from_char('z'), None);
    assert_eq!(PaletteRoute::from_char(' '), None);
    assert_eq!(PaletteRoute::from_char('@'), None); // reserved for symbols
}

#[test]
fn palette_route_label() {
    assert_eq!(PaletteRoute::Commands.label(), "Commands");
    assert_eq!(PaletteRoute::Files.label(), "Files");
    assert_eq!(PaletteRoute::Content.label(), "Content");
    assert_eq!(PaletteRoute::GotoLine.label(), "Go to Line");
}

#[test]
fn palette_route_prefix_char() {
    assert_eq!(PaletteRoute::Commands.prefix_char(), '>');
    assert_eq!(PaletteRoute::Files.prefix_char(), '/');
    assert_eq!(PaletteRoute::Content.prefix_char(), '#');
    assert_eq!(PaletteRoute::GotoLine.prefix_char(), ':');
}

#[test]
fn starts_in_commands_route() {
    let p = CommandPalette::default();
    assert_eq!(p.route, PaletteRoute::Commands);
    assert!(p.route_search.is_none());
    assert!(p.route_goto_line.is_none());
}

#[test]
fn slash_prefix_switches_to_files_route() {
    let mut p = CommandPalette::default();
    p.push('/');
    assert_eq!(p.route, PaletteRoute::Files);
    // No sub-picker created by push alone — that's done by the key handler.
    assert!(p.route_search.is_none());
}

#[test]
fn hash_prefix_switches_to_content_route() {
    let mut p = CommandPalette::default();
    p.push('#');
    assert_eq!(p.route, PaletteRoute::Content);
}

#[test]
fn colon_prefix_switches_to_goto_line_route() {
    let mut p = CommandPalette::default();
    p.push(':');
    assert_eq!(p.route, PaletteRoute::GotoLine);
    // No sub-picker created by push alone.
    assert!(p.route_goto_line.is_none());
}

#[test]
fn gt_prefix_stays_in_commands_route() {
    let mut p = CommandPalette::default();
    p.push('>');
    assert_eq!(p.route, PaletteRoute::Commands);
}

#[test]
fn non_prefix_first_char_stays_in_commands() {
    let mut p = CommandPalette::default();
    p.push('h');
    assert_eq!(p.route, PaletteRoute::Commands);
    assert_eq!(p.query, "h");
}

#[test]
fn pop_returns_to_commands_from_routed_mode() {
    let mut p = CommandPalette::default();
    p.push('/');
    assert_eq!(p.route, PaletteRoute::Files);
    // Pop when sub-query is empty should return to commands.
    p.pop();
    assert_eq!(p.route, PaletteRoute::Commands);
}

#[test]
fn pop_keeps_route_when_routed_query_becomes_empty() {
    let mut p = CommandPalette::default();
    p.push(':');
    p.route_goto_line = Some(crate::search::GotoLineState::new());
    p.push('4');
    p.pop();
    // Deleting the last typed char keeps the route; only a further backspace
    // on the now-empty query (the dispatcher's Close) returns to commands.
    assert_eq!(p.route, PaletteRoute::GotoLine);
    assert!(p.route_goto_line.is_some());
    assert!(p.is_query_empty());
}

#[test]
fn active_query_returns_command_query_in_commands_mode() {
    let mut p = CommandPalette::default();
    p.push('t');
    assert_eq!(p.active_query(), "t");
}

#[test]
fn is_query_empty_reflects_command_query() {
    let mut p = CommandPalette::default();
    assert!(p.is_query_empty());
    p.push('t');
    assert!(!p.is_query_empty());
    p.pop();
    assert!(p.is_query_empty());
}

#[test]
fn list_picker_query_is_empty_delegates_to_route() {
    use crate::list_picker::ListPicker;
    let mut p = CommandPalette::default();
    assert!(ListPicker::query_is_empty(&p));
    // In commands mode, push a char makes query non-empty.
    ListPicker::query_push(&mut p, 't');
    assert!(!ListPicker::query_is_empty(&p));
    ListPicker::query_pop(&mut p);
    assert!(ListPicker::query_is_empty(&p));

    // In files route (no sub-picker yet), query is empty.
    ListPicker::query_push(&mut p, '/');
    assert!(ListPicker::query_is_empty(&p));
}
