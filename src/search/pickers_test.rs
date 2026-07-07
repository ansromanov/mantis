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

// -- CompareModeInput ---------------------------------------------------------

#[test]
fn compare_mode_input_new_is_empty() {
    let s = CompareModeInput::new();
    assert!(s.query.is_empty());
}

#[test]
fn compare_mode_input_default_is_empty() {
    let s = CompareModeInput::default();
    assert!(s.query.is_empty());
}

#[test]
fn compare_mode_input_push_appends() {
    let mut s = CompareModeInput::new();
    s.push('H');
    s.push('E');
    s.push('A');
    s.push('D');
    assert_eq!(s.query, "HEAD");
}

#[test]
fn compare_mode_input_pop_removes() {
    let mut s = CompareModeInput::new();
    s.push('a');
    s.push('b');
    s.pop();
    assert_eq!(s.query, "a");
    s.pop();
    assert!(s.query.is_empty());
}

#[test]
fn compare_mode_input_list_picker_has_no_selectable_results() {
    let mut s = CompareModeInput::new();
    assert_eq!(ListPicker::results_len(&s), 0);
    assert_eq!(ListPicker::selected(&s), 0);
    ListPicker::set_selected(&mut s, 5);
    assert_eq!(ListPicker::selected(&s), 0, "set_selected is a no-op");
}

#[test]
fn compare_mode_input_list_picker_query_methods_delegate() {
    let mut s = CompareModeInput::new();
    assert!(ListPicker::query_is_empty(&s));
    ListPicker::query_push(&mut s, 'x');
    assert_eq!(s.query, "x");
    assert!(!ListPicker::query_is_empty(&s));
    ListPicker::query_pop(&mut s);
    assert!(s.query.is_empty());
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
