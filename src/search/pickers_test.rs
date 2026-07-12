use super::*;

use std::path::PathBuf;

// -- InFileSearch ----------------------------------------------------------

#[test]
fn in_file_search_finds_matches() {
    let mut s = InFileSearch::new();
    assert!(s.matches.is_empty());
    assert_eq!(s.current, 0);

    let lines = ["hello world".to_string(), "foo bar".to_string()];
    s.push('o');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 4);

    s.push(' ');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 2);
    assert_eq!(s.matches[0].line, 0);
    assert_eq!(s.matches[0].col, 4);

    s.pop();
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 4);
}

#[test]
fn in_file_search_case_insensitive() {
    let mut s = InFileSearch::new();
    let lines = ["Hello World".to_string()];
    s.push('w');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 1);
    assert_eq!(s.matches[0].col, 6);
}

#[test]
fn in_file_search_empty_query_clears_matches() {
    let mut s = InFileSearch::new();
    let lines = ["hello".to_string()];
    s.push('h');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 1);
    s.pop();
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert!(s.matches.is_empty());
}

#[test]
fn in_file_search_current_navigation() {
    let mut s = InFileSearch::new();
    let lines = ["aa".to_string()];
    s.push('a');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 2);
    assert_eq!(s.current, 0);
}

#[test]
fn in_file_search_default_is_empty() {
    let s = InFileSearch::default();
    assert!(s.query.is_empty());
    assert!(s.matches.is_empty());
    assert_eq!(s.current, 0);
}

#[test]
fn in_file_search_case_sensitive() {
    let mut s = InFileSearch::new();
    let lines = ["HelloWorld".to_string(), "helloworld".to_string()];
    s.push('W');
    s.push('o');
    s.push('r');
    s.push('l');
    s.push('d');

    // Case-insensitive by default
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 2);

    // Case-sensitive enabled
    s.case_sensitive = true;
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 1);
    assert_eq!(s.matches[0].line, 0);
    assert_eq!(s.matches[0].col, 5);
}

#[test]
fn in_file_search_regex() {
    let mut s = InFileSearch::new();
    let lines = ["abc123xyz".to_string(), "abc456xyz".to_string()];
    s.push('a');
    s.push('b');
    s.push('c');
    s.push('[');
    s.push('0');
    s.push('-');
    s.push('9');
    s.push(']');
    s.push('+');
    s.push('x');
    s.push('y');
    s.push('z');

    // literal by default (the character class is not interpreted)
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 0);

    // regex enabled
    s.regex = true;
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 2);
}

#[test]
fn in_file_search_whole_word() {
    let mut s = InFileSearch::new();
    let lines = ["hello world".to_string(), "helloworld".to_string()];
    s.push('h');
    s.push('e');
    s.push('l');
    s.push('l');
    s.push('o');

    // substring by default
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 2);

    // whole word enabled
    s.whole_word = true;
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 1);
    assert_eq!(s.matches[0].line, 0);
    assert_eq!(s.matches[0].col, 0);
}

#[test]
fn in_file_search_regex_skips_zero_length_matches() {
    let mut s = InFileSearch::new();
    let lines = ["aaa bbb".to_string()];
    s.regex = true;
    // `a*` matches the empty string at every position; only the non-empty
    // "aaa" run should be recorded.
    s.push('a');
    s.push('*');
    s.refresh(lines.len(), |i| lines.get(i).cloned());
    assert_eq!(s.matches.len(), 1);
    assert_eq!(s.matches[0].col, 0);
    assert_eq!(s.matches[0].len, 3);
}

// -- ThemePicker -----------------------------------------------------------

#[test]
fn theme_picker_starts_with_all_presets() {
    let p = ThemePicker::default();
    let count = crate::theme::Theme::discover_all().len();
    assert_eq!(p.names.len(), count);
    assert_eq!(p.filtered.len(), count);
    assert_eq!(p.selected, 0);
}

#[test]
fn theme_picker_push_filters() {
    let mut p = ThemePicker::default();
    p.push('m');
    assert!(p.filtered.len() < p.names.len());
    assert!(p.filtered.iter().any(|&i| p.names[i].contains("monokai")));
}

#[test]
fn theme_picker_pop_restores() {
    let mut p = ThemePicker::default();
    p.push('m');
    let filtered_after_push = p.filtered.len();
    p.pop();
    assert_eq!(p.filtered.len(), p.names.len());
    assert!(filtered_after_push < p.names.len());
}

#[test]
fn theme_picker_selected_name() {
    let mut p = ThemePicker::default();
    p.push('m');
    let name = p.selected_name();
    assert!(name.is_some());
    assert!(name.unwrap().contains("monokai"));
}

#[test]
fn theme_picker_selected_name_returns_none_when_empty() {
    let mut p = ThemePicker::default();
    for c in "zzzzzzz".chars() {
        p.push(c);
    }
    assert_eq!(p.results_len(), 0);
    assert!(p.selected_name().is_none());
}

#[test]
fn theme_picker_results_len() {
    let p = ThemePicker::default();
    assert_eq!(p.results_len(), crate::theme::Theme::discover_all().len());
}

#[test]
fn theme_picker_themes_parallel_names() {
    let p = ThemePicker::default();
    assert_eq!(p.names.len(), p.themes.len());
}

#[test]
fn theme_picker_selected_theme_matches_selected_name() {
    let mut p = ThemePicker::default();
    p.push('m');
    let name = p.selected_name().unwrap().to_string();
    let expected = crate::theme::Theme::load(&name).unwrap();
    let got = p.selected_theme().unwrap();
    assert_eq!(got.accent, expected.accent);
    assert_eq!(got.background, expected.background);
}

#[test]
fn theme_picker_selected_theme_returns_none_when_empty() {
    let mut p = ThemePicker::default();
    for c in "zzzzzzz".chars() {
        p.push(c);
    }
    assert!(p.selected_theme().is_none());
}

// -- RecentFilesState -------------------------------------------------------

fn sample_paths() -> Vec<PathBuf> {
    vec![
        PathBuf::from("/tmp/alpha.rs"),
        PathBuf::from("/tmp/beta.rs"),
        PathBuf::from("/tmp/gamma.toml"),
    ]
}

#[test]
fn recent_files_state_starts_with_all_paths() {
    let r = RecentFilesState::new(sample_paths());
    assert_eq!(r.results_len(), 3);
    assert_eq!(r.selected, 0);
    assert!(r.query.is_empty());
}

#[test]
fn recent_files_state_push_filters() {
    let mut r = RecentFilesState::new(sample_paths());
    // 'h' only appears in "/tmp/alpha.rs" (not in beta or gamma)
    r.push('h');
    assert!(r.results_len() < 3);
    let path = r.selected_path().unwrap();
    assert!(path.to_string_lossy().contains('h'));
}

#[test]
fn recent_files_state_pop_restores() {
    let mut r = RecentFilesState::new(sample_paths());
    r.push('h');
    let after_push = r.results_len();
    r.pop();
    assert_eq!(r.results_len(), 3);
    assert!(after_push < 3);
}

#[test]
fn recent_files_state_selected_path_in_bounds() {
    let mut r = RecentFilesState::new(sample_paths());
    assert_eq!(r.selected_path().unwrap(), &PathBuf::from("/tmp/alpha.rs"));
    r.selected = 2;
    assert_eq!(
        r.selected_path().unwrap(),
        &PathBuf::from("/tmp/gamma.toml")
    );
}

#[test]
fn recent_files_state_selected_path_returns_none_when_empty() {
    let paths: Vec<PathBuf> = vec![];
    let r = RecentFilesState::new(paths);
    assert!(r.selected_path().is_none());
}

#[test]
fn recent_files_state_selected_path_returns_none_out_of_bounds() {
    let mut r = RecentFilesState::new(sample_paths());
    r.selected = 99;
    assert!(r.selected_path().is_none());
}

#[test]
fn recent_files_state_no_match_gives_empty_results() {
    let mut r = RecentFilesState::new(sample_paths());
    for c in "zzzzzzz".chars() {
        r.push(c);
    }
    assert_eq!(r.results_len(), 0);
    assert!(r.selected_path().is_none());
}

#[test]
fn recent_files_list_picker_impl_delegates() {
    use crate::list_picker::ListPicker;
    let paths = vec![
        PathBuf::from("/a.txt"),
        PathBuf::from("/b.txt"),
        PathBuf::from("/c.txt"),
    ];
    let mut r = RecentFilesState::new(paths);
    assert_eq!(ListPicker::results_len(&r), 3);
    assert_eq!(ListPicker::selected(&r), 0);
    ListPicker::set_selected(&mut r, 2);
    assert_eq!(r.selected, 2);
    assert!(ListPicker::query_is_empty(&r));
    ListPicker::query_push(&mut r, 'a');
    assert!(!ListPicker::query_is_empty(&r));
    assert!(
        ListPicker::results_len(&r) < 3,
        "push should filter results"
    );
    ListPicker::query_pop(&mut r);
    assert!(ListPicker::query_is_empty(&r));
    assert_eq!(ListPicker::results_len(&r), 3);
}

// -- GotoLineState -----------------------------------------------------------

#[test]
fn goto_line_state_new_is_empty() {
    let s = GotoLineState::new();
    assert!(s.query.is_empty());
}

#[test]
fn goto_line_state_push_appends() {
    let mut s = GotoLineState::new();
    s.push('4');
    assert_eq!(s.query, "4");
    s.push('2');
    assert_eq!(s.query, "42");
}

#[test]
fn goto_line_state_pop_removes() {
    let mut s = GotoLineState::new();
    s.push('4');
    s.push('2');
    s.pop();
    assert_eq!(s.query, "4");
    s.pop();
    assert!(s.query.is_empty());
}

#[test]
fn goto_line_state_default_is_empty() {
    let s = GotoLineState::default();
    assert!(s.query.is_empty());
}

// -- RevisionPicker -----------------------------------------------------------

#[test]
fn revision_picker_new_with_nonexistent_repo_has_shortcuts_only() {
    let p = RevisionPicker::new(std::path::Path::new("/nonexistent"));
    assert_eq!(p.query, "");
    assert!(
        p.shortcuts.iter().any(|i| i.rev == "HEAD"),
        "HEAD shortcut must always be present"
    );
    assert!(
        p.shortcuts.iter().any(|i| i.rev == "HEAD~1"),
        "HEAD~1 shortcut must always be present"
    );
}

#[test]
fn revision_picker_push_appends() {
    let mut p = RevisionPicker::for_test(vec![RevisionItem {
        rev: "HEAD".into(),
        display: "HEAD (current)".into(),
    }]);
    p.push('H');
    assert_eq!(p.query, "H");
}

#[test]
fn revision_picker_pop_removes() {
    let mut p = RevisionPicker::for_test(vec![RevisionItem {
        rev: "HEAD".into(),
        display: "HEAD (current)".into(),
    }]);
    p.query = "HE".to_string();
    p.pop();
    assert_eq!(p.query, "H");
    p.pop();
    assert!(p.query.is_empty());
}

#[test]
fn revision_picker_selected_rev_returns_correct_rev() {
    let mut p = RevisionPicker::for_test(vec![
        RevisionItem {
            rev: "HEAD".into(),
            display: "HEAD (current)".into(),
        },
        RevisionItem {
            rev: "abc1234".into(),
            display: "abc1234 fix".into(),
        },
    ]);
    assert_eq!(p.selected_rev(), Some("HEAD"));
    p.selected = 1;
    assert_eq!(p.selected_rev(), Some("abc1234"));
}

#[test]
fn revision_picker_list_picker_query_methods_delegate() {
    let mut p = RevisionPicker::for_test(vec![RevisionItem {
        rev: "HEAD".into(),
        display: "HEAD (current)".into(),
    }]);
    assert!(ListPicker::query_is_empty(&p));
    ListPicker::query_push(&mut p, 'x');
    assert_eq!(p.query, "x");
    assert!(!ListPicker::query_is_empty(&p));
    ListPicker::query_pop(&mut p);
    assert!(ListPicker::query_is_empty(&p));
}

#[test]
fn revision_picker_refilter_filters_by_display_text() {
    let mut p = RevisionPicker::for_test(vec![
        RevisionItem {
            rev: "HEAD".into(),
            display: "HEAD (current)".into(),
        },
        RevisionItem {
            rev: "main".into(),
            display: "branch: main".into(),
        },
        RevisionItem {
            rev: "abc1234".into(),
            display: "abc1234 fix bug".into(),
        },
    ]);
    p.query = "main".to_string();
    p.refilter();
    assert_eq!(p.results_len(), 1);
    assert_eq!(p.selected_rev(), Some("main"));
}

#[test]
fn revision_picker_next_tab_cycles_through_tabs() {
    let mut p = RevisionPicker::for_test(vec![]);
    assert_eq!(p.tab, RevisionTab::Commits);
    p.next_tab();
    assert_eq!(p.tab, RevisionTab::Tags);
    p.next_tab();
    assert_eq!(p.tab, RevisionTab::Branches);
    p.next_tab();
    assert_eq!(p.tab, RevisionTab::Commits);
}

#[test]
fn revision_picker_prev_tab_cycles_in_reverse() {
    let mut p = RevisionPicker::for_test(vec![]);
    assert_eq!(p.tab, RevisionTab::Commits);
    p.prev_tab();
    assert_eq!(p.tab, RevisionTab::Branches);
    p.prev_tab();
    assert_eq!(p.tab, RevisionTab::Tags);
    p.prev_tab();
    assert_eq!(p.tab, RevisionTab::Commits);
}

#[test]
fn revision_picker_tab_label() {
    assert_eq!(RevisionTab::Commits.label(), "Commits");
    assert_eq!(RevisionTab::Tags.label(), "Tags");
    assert_eq!(RevisionTab::Branches.label(), "Branches");
}

#[test]
fn revision_picker_switch_tab_rebuilds_items() {
    let mut p = RevisionPicker {
        items: Vec::new(),
        query: String::new(),
        filtered: Vec::new(),
        selected: 0,
        matcher: SkimMatcherV2::default(),
        tab: RevisionTab::Commits,
        shortcuts: vec![RevisionItem {
            rev: "HEAD".into(),
            display: "HEAD (current)".into(),
        }],
        commits: vec![RevisionItem {
            rev: "abc1234".into(),
            display: "abc1234 fix".into(),
        }],
        tags: vec![RevisionItem {
            rev: "v1.0".into(),
            display: "v1.0".into(),
        }],
        branches: vec![RevisionItem {
            rev: "main".into(),
            display: "main".into(),
        }],
    };
    p.rebuild_items();
    // Commits tab: shortcuts + commits
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].rev, "HEAD");
    assert_eq!(p.items[1].rev, "abc1234");

    p.next_tab(); // Tags
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].rev, "HEAD");
    assert_eq!(p.items[1].rev, "v1.0");

    p.next_tab(); // Branches
    assert_eq!(p.items.len(), 2);
    assert_eq!(p.items[0].rev, "HEAD");
    assert_eq!(p.items[1].rev, "main");
}

// -- TreeFilter --------------------------------------------------------------

#[test]
fn tree_filter_new_has_no_cache() {
    let f = TreeFilter::new();
    assert!(f.cached.is_none());
    assert!(f.saved_expanded.is_none());
    assert!(f.full_paths_cache.is_none());
}

#[test]
fn tree_filter_push_invalidates_cache() {
    let mut f = TreeFilter::new();
    f.cached = Some(("a".to_string(), 0, vec![1, 2, 3]));
    f.push('b');
    assert!(f.cached.is_none(), "push must clear the filter cache");
}

#[test]
fn tree_filter_pop_invalidates_cache() {
    let mut f = TreeFilter::new();
    f.push('a');
    f.cached = Some(("a".to_string(), 0, vec![1, 2, 3]));
    f.pop();
    assert!(f.cached.is_none(), "pop must clear the filter cache");
}

#[test]
fn tree_filter_regex_matches_via_regex() {
    let mut f = TreeFilter::new();
    f.push('r');
    f.push('?');
    f.push('s');
    f.push('r');
    f.push('c');
    // "r?src" matches "src" (the `r?` part is optional) via regex
    assert!(f.regex.is_some(), "a valid regex should be compiled");
    assert!(f.matches_name("src"));
    assert!(f.matches_name("rsrc"));
    assert!(!f.matches_name("source"));
}

#[test]
fn tree_filter_invalid_regex_falls_back_to_substring() {
    let mut f = TreeFilter::new();
    f.push('[');
    // Unclosed bracket is not a valid regex
    assert!(f.regex.is_none(), "invalid regex should fall back to None");
    // Falls back to literal substring match: "[" is contained in "file[1]"
    assert!(f.matches_name("file[1]"));
    assert!(!f.matches_name("file1"));
}

#[test]
fn tree_filter_regex_case_insensitive() {
    let mut f = TreeFilter::new();
    for c in "README".chars() {
        f.push(c);
    }
    assert!(f.matches_name("readme"));
    assert!(f.matches_name("README"));
    assert!(f.matches_name("Readme.md"));
    assert!(!f.matches_name("read"));
}

#[test]
fn tree_filter_regex_empty_query_matches_all() {
    let f = TreeFilter::new();
    assert!(f.matches_name("anything"));
    assert!(f.matches_name(""));
}

#[test]
fn tree_filter_substring_fallback_case_insensitive() {
    let mut f = TreeFilter::new();
    // A pattern that won't compile as regex but is a valid substring
    f.push('(');
    f.push('?');
    // `(?` is not a valid regex by itself but `(?` is valid as substring
    // Actually `(?` is a valid regex start of a group. Let's use a pattern that's
    // definitely not regex but is a valid substring.
    // Just use plain substring matching with an unclosed group
    let mut g = TreeFilter::new();
    g.push('[');
    g.push('a');
    g.push('b');
    // `[ab` is not a valid regex (unclosed character class)
    assert!(g.regex.is_none());
    assert!(g.matches_name("foo[ab]"));
    assert!(!g.matches_name("fooab"));
}

#[test]
fn tree_filter_new_has_no_regex() {
    let f = TreeFilter::new();
    assert!(f.regex.is_none());
}

#[test]
fn tree_filter_push_rebuilds_regex() {
    let mut f = TreeFilter::new();
    f.push('a');
    assert!(f.regex.is_some(), "a single char is a valid regex");
    f.push('*');
    assert!(f.regex.is_some(), "'a*' is also valid regex");
}

#[test]
fn tree_filter_pop_rebuilds_regex() {
    let mut f = TreeFilter::new();
    f.push('a');
    f.push('\\'); // "a\" - trailing backslash is invalid regex
    assert!(f.regex.is_none(), "'a\\' is an invalid regex");
    f.push('\\'); // "a\\" - escaped backslash is valid
    assert!(f.regex.is_some());
    f.pop(); // back to "a\" - invalid again
    assert!(f.regex.is_none(), "'a\\' trailing backslash is invalid");
}

// -- PluginPicker ------------------------------------------------------------

#[test]
fn plugin_picker_new_stores_entries_and_starts_selected_at_zero() {
    let entries = vec![
        (
            "alpha".to_string(),
            true,
            crate::plugin::PluginKind::Process,
            None,
        ),
        (
            "beta".to_string(),
            false,
            crate::plugin::PluginKind::Process,
            Some("panic: oh no".to_string()),
        ),
    ];
    let picker = PluginPicker::new(entries.clone());
    assert_eq!(picker.entries, entries);
    assert_eq!(picker.selected, 0);
}

#[test]
fn plugin_picker_entries_carry_crash_badge_only_for_dead_plugins() {
    let picker = PluginPicker::new(vec![
        (
            "running".to_string(),
            true,
            crate::plugin::PluginKind::Process,
            None,
        ),
        (
            "crashed".to_string(),
            false,
            crate::plugin::PluginKind::Process,
            Some("exited unexpectedly".to_string()),
        ),
    ]);
    assert_eq!(picker.entries[0].3, None);
    assert_eq!(picker.entries[1].3.as_deref(), Some("exited unexpectedly"));
}

// -- fuzzy_refilter integration (new return type) -----------------------------

#[test]
fn theme_picker_refilter_handles_new_return_type() {
    let mut p = ThemePicker::default();
    let total = p.names.len();
    p.push('m');
    assert!(p.filtered.len() < total);
    // Should still work after pop
    p.pop();
    assert_eq!(p.filtered.len(), total);
}

#[test]
fn recent_files_refilter_handles_new_return_type() {
    let paths = vec![
        std::path::PathBuf::from("/tmp/alpha.rs"),
        std::path::PathBuf::from("/tmp/beta.rs"),
    ];
    let mut r = RecentFilesState::new(paths);
    let total = r.paths.len();
    // Filtering should still work
    r.push('z');
    assert_eq!(r.results_len(), 0);
    r.pop();
    assert_eq!(r.results_len(), total);
}

#[test]
fn bug_report_state_editing_operations() {
    let mut state = BugReportState::default();
    assert_eq!(state.text, vec![""]);
    assert_eq!(state.cursor_row, 0);
    assert_eq!(state.cursor_col, 0);

    // Test insert_char
    state.insert_char('A');
    state.insert_char('B');
    assert_eq!(state.text, vec!["AB"]);
    assert_eq!(state.cursor_col, 2);

    // Test insert_newline
    state.insert_newline();
    assert_eq!(state.text, vec!["AB".to_string(), "".to_string()]);
    assert_eq!(state.cursor_row, 1);
    assert_eq!(state.cursor_col, 0);

    // Test insert_char in second line
    state.insert_char('C');
    assert_eq!(state.text, vec!["AB".to_string(), "C".to_string()]);

    // Test backspace within line
    state.backspace();
    assert_eq!(state.text, vec!["AB".to_string(), "".to_string()]);

    // Test backspace to merge lines
    state.backspace();
    assert_eq!(state.text, vec!["AB".to_string()]);
    assert_eq!(state.cursor_row, 0);
    assert_eq!(state.cursor_col, 2);

    // Test backspace at start of single line
    state.cursor_col = 0;
    state.backspace();
    assert_eq!(state.text, vec!["AB".to_string()]);

    // Test move_left / move_right
    state.cursor_col = 1;
    state.move_left();
    assert_eq!(state.cursor_col, 0);
    state.move_right();
    assert_eq!(state.cursor_col, 1);

    // Test delete within line
    state.delete();
    assert_eq!(state.text, vec!["A".to_string()]);

    // Test cursor vertical movement
    state.insert_char('X'); // "AX"
    state.insert_newline(); // line 0: "AX", line 1: ""
    state.insert_char('Y'); // line 1: "Y"
    state.move_up();
    assert_eq!(state.cursor_row, 0);
    assert_eq!(state.cursor_col, 1); // clamped to length of "AX" (which is 2)
    state.move_down();
    assert_eq!(state.cursor_row, 1);

    // Test move_home and move_end
    state.move_home();
    assert_eq!(state.cursor_col, 0);
    state.move_end();
    assert_eq!(state.cursor_col, 1); // length of "Y"
}

#[test]
fn bug_report_state_custom_diagnostics() {
    let state = BugReportState::new("test diagnostics info".to_string());
    assert_eq!(state.diagnostics_markdown, "test diagnostics info");
}

#[test]
fn bug_report_state_total_visual_rows() {
    let mut state = BugReportState::default();
    // Single empty line -> 1 visual row
    assert_eq!(state.total_visual_rows(10), 1);

    // Short line -> 1 visual row
    state.insert_char('a');
    state.insert_char('b');
    assert_eq!(state.total_visual_rows(10), 1);

    // Line that fills exactly one chunk
    state = BugReportState::default();
    for _ in 0..10 {
        state.insert_char('x');
    }
    // Cursor at index 10 wraps to the next line, so total is 2
    assert_eq!(state.total_visual_rows(10), 2);
    // If cursor is moved back, no extra row is needed
    state.cursor_col = 9;
    assert_eq!(state.total_visual_rows(10), 1);
    // Restore cursor position for subsequent insertions in the test
    state.cursor_col = 10;

    // Line that wraps to 2 visual rows
    for _ in 0..5 {
        state.insert_char('y');
    }
    assert_eq!(state.total_visual_rows(10), 2);

    // Multiple lines with wrapping
    let mut state = BugReportState::default();
    // line 0: 25 chars -> 3 visual rows at width=10
    for _ in 0..25 {
        state.insert_char('a');
    }
    state.insert_newline();
    // line 1: 3 chars -> 1 visual row
    state.insert_char('b');
    state.insert_char('c');
    state.insert_char('d');
    assert_eq!(state.total_visual_rows(10), 4);
}

#[test]
fn bug_report_state_cursor_visual_row_single_short_line() {
    let mut state = BugReportState::default();
    state.insert_char('H');
    state.insert_char('i');
    // cursor_col=2, single line with 2 chars, width=10 -> 1 visual row
    assert_eq!(state.cursor_visual_row(10), 0);
}

#[test]
fn bug_report_state_cursor_visual_row_wrapped_line() {
    let mut state = BugReportState::default();
    // Line with 15 chars
    for _ in 0..15 {
        state.insert_char('x');
    }
    // cursor_col=15, width=10 -> visual row = 0 + 15/10 = 1
    assert_eq!(state.cursor_visual_row(10), 1);

    // cursor_col=9 -> visual row = 0 + 9/10 = 0
    state.cursor_col = 9;
    assert_eq!(state.cursor_visual_row(10), 0);

    // cursor_col=10 -> visual row = 0 + 10/10 = 1
    state.cursor_col = 10;
    assert_eq!(state.cursor_visual_row(10), 1);
}

#[test]
fn bug_report_state_cursor_visual_row_multi_line() {
    let mut state = BugReportState::default();
    // line 0: 20 chars -> 2 visual rows at width=10
    for _ in 0..20 {
        state.insert_char('a');
    }
    state.insert_newline();
    // line 1: 5 chars -> 1 visual row
    for _ in 0..5 {
        state.insert_char('b');
    }
    // cursor at line 1, col 3, width=10
    // visual = visual_rows("aaa...") = 2 + 3/10 = 2
    assert_eq!(state.cursor_visual_row(10), 2);

    // cursor at line 0, col 15 -> visual = 0 + 15/10 = 1
    state.cursor_row = 0;
    state.cursor_col = 15;
    assert_eq!(state.cursor_visual_row(10), 1);
}

#[test]
fn bug_report_state_clamp_scroll_wrapped_lines() {
    let mut state = BugReportState::default();
    // line 0: 25 chars -> 3 visual rows at width=10
    for _ in 0..25 {
        state.insert_char('a');
    }
    // cursor_visual_row(10) = 0 + 25/10 = 2
    // height=2, width=10: scroll_top should be 1 (cursor_vis - height + 1 = 2 - 2 + 1 = 1)
    state.clamp_scroll(2, 10);
    assert_eq!(state.scroll_top, 1);

    // With height=3, cursor at vis=2: scroll_top stays 0 (cursor visible)
    state.scroll_top = 0;
    state.clamp_scroll(3, 10);
    assert_eq!(state.scroll_top, 0);

    // Cursor below scroll_top: scroll_top should move down to keep cursor visible
    state.scroll_top = 0;
    state.clamp_scroll(1, 10);
    // cursor_vis=2, height=1 -> scroll_top = 2 - 1 + 1 = 2
    assert_eq!(state.scroll_top, 2);
}

#[test]
fn bug_report_state_clamp_scroll_wrapped_lines_multi_line() {
    let mut state = BugReportState::default();
    // line 0: 25 chars -> 3 visual rows
    for _ in 0..25 {
        state.insert_char('a');
    }
    state.insert_newline();
    // line 1: 25 chars -> 3 visual rows (total = 6)
    for _ in 0..25 {
        state.insert_char('b');
    }
    // After insertions cursor is at line 1, col 25 (end), width=10
    // cursor_vis = 3 (line 0) + 25/10 = 3 + 2 = 5
    // height=2: scroll_top = 5 - 2 + 1 = 4, max_scroll = 6 - 2 = 4
    state.clamp_scroll(2, 10);
    assert_eq!(state.scroll_top, 4);

    // Move cursor to start of line 1 -> cursor_vis = 3 + 0/10 = 3
    // cursor_vis < scroll_top (4) -> scroll_top slides up to cursor_vis = 3
    state.cursor_col = 0;
    state.clamp_scroll(2, 10);
    assert_eq!(state.scroll_top, 3);
}

#[test]
fn bug_report_state_clamp_scroll_zero_height_does_nothing() {
    let mut state = BugReportState::default();
    state.insert_char('x');
    state.scroll_top = 42;
    state.clamp_scroll(0, 10);
    assert_eq!(state.scroll_top, 42);

    state.clamp_scroll(3, 0);
    assert_eq!(state.scroll_top, 42);
}

#[test]
fn bug_report_state_clamp_scroll_empty_text() {
    let mut state = BugReportState::default();
    state.clamp_scroll(3, 10);
    assert_eq!(state.scroll_top, 0);
}
