use super::*;

use std::sync::atomic::AtomicUsize;
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn search_temp_dir(label: &str) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("tv_search_{}_{}_{}", label, std::process::id(), n))
}

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

// -- HistoryState ----------------------------------------------------------

fn sample_commits() -> Vec<crate::git::Commit> {
    vec![
        crate::git::Commit {
            hash: "abc123def456".into(),
            short: "abc123".into(),
            date: "2024-01-15".into(),
            subject: "fix critical bug".into(),
        },
        crate::git::Commit {
            hash: "def789abc012".into(),
            short: "def789".into(),
            date: "2024-01-14".into(),
            subject: "add new feature".into(),
        },
        crate::git::Commit {
            hash: "ghi345jkl678".into(),
            short: "ghi345".into(),
            date: "2024-01-13".into(),
            subject: "refactor module".into(),
        },
    ]
}

#[test]
fn history_state_starts_with_all_commits() {
    let commits = sample_commits();
    let h = HistoryState::new(PathBuf::from("f.txt"), commits);
    assert_eq!(h.results_len(), 3);
    assert_eq!(h.selected, 0);
}

#[test]
fn history_state_push_filters() {
    let commits = sample_commits();
    let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
    h.push('b');
    assert!(h.results_len() < 3);
    assert_eq!(h.filtered[0], 0);
}

#[test]
fn history_state_pop_restores() {
    let commits = sample_commits();
    let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
    h.push('b');
    let after_push = h.results_len();
    h.pop();
    assert_eq!(h.results_len(), 3);
    assert!(after_push < 3);
}

#[test]
fn history_state_selected_commit() {
    let commits = sample_commits();
    let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
    assert_eq!(h.selected_commit().unwrap().short, "abc123");
    h.selected = 1;
    assert_eq!(h.selected_commit().unwrap().short, "def789");
}

#[test]
fn history_state_selected_commit_returns_none_out_of_bounds() {
    let commits = sample_commits();
    let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
    h.selected = 99;
    assert!(h.selected_commit().is_none());
}

#[test]
fn history_state_filtered_out_of_bounds() {
    let commits = sample_commits();
    let mut h = HistoryState::new(PathBuf::from("f.txt"), commits);
    for c in "zzzzzzz".chars() {
        h.push(c);
    }
    assert_eq!(h.results_len(), 0);
    assert!(h.selected_commit().is_none());
}

// -- SearchState -----------------------------------------------------------

#[test]
fn search_state_new_creates_file_results() {
    let root = search_temp_dir("new");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello\n").unwrap();
    fs::write(root.join("b.txt"), "world\n").unwrap();

    let s = SearchState::new(&root, false, true, 0, None);
    assert_eq!(s.file_results.len(), 2);
    assert_eq!(s.mode, SearchMode::Files);
    assert!(s.query.is_empty());
    assert_eq!(s.selected, 0);
    assert!(!s.scoped);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_new_scoped() {
    let mut files = std::collections::HashSet::new();
    files.insert(PathBuf::from("/tmp/a.txt"));
    files.insert(PathBuf::from("/tmp/b.txt"));
    let s = SearchState::new(Path::new("/tmp"), false, true, 0, Some(&files));
    assert_eq!(s.all_files.len(), 2);
    assert!(s.scoped);
}

#[test]
fn search_state_new_scoped_filters_outside_root() {
    let mut files = std::collections::HashSet::new();
    files.insert(PathBuf::from("/tmp/a.txt"));
    files.insert(PathBuf::from("/other/b.txt"));
    let s = SearchState::new(Path::new("/tmp"), false, true, 0, Some(&files));
    assert_eq!(s.all_files.len(), 1);
    assert!(s.scoped);
}

#[test]
fn search_state_new_scoped_empty_set() {
    let files = std::collections::HashSet::new();
    let s = SearchState::new(Path::new("/tmp"), false, true, 0, Some(&files));
    assert!(s.all_files.is_empty());
    assert!(s.scoped);
}

#[test]
fn search_state_push_and_pop_query() {
    let root = search_temp_dir("push_pop");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello\n").unwrap();
    fs::write(root.join("b.txt"), "world\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    assert_eq!(s.file_results.len(), 2);
    s.push('A');
    assert_eq!(s.query, "A");
    s.pop();
    assert_eq!(s.query, "");
    assert_eq!(s.file_results.len(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_toggle_mode() {
    let root = search_temp_dir("toggle");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    assert_eq!(s.mode, SearchMode::Files);
    s.toggle_mode();
    assert_eq!(s.mode, SearchMode::Content);
    s.toggle_mode();
    assert_eq!(s.mode, SearchMode::Files);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_results_len() {
    let root = search_temp_dir("results_len");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    assert_eq!(s.results_len(), 1);
    s.toggle_mode();
    s.push('h');
    assert_eq!(s.results_len(), 0);
    s.push('e');
    s.refresh_now();
    assert_eq!(s.results_len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_content_context_lines() {
    let root = search_temp_dir("context");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("a.yaml"),
        "database:\n  host: db.internal\n  port: 5432\n",
    )
    .unwrap();

    let mut s = SearchState::new(&root, false, true, 2, None);
    s.toggle_mode();
    s.push('d');
    s.push('a');
    s.push('t');
    s.refresh_now();
    assert_eq!(s.content_results.len(), 1);
    assert_eq!(s.content_results[0].context.len(), 2);
    assert!(s.content_results[0].context[0].contains("host"));
    assert!(s.content_results[0].context[1].contains("port"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_content_context_capped_at_eof() {
    let root = search_temp_dir("context_eof");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "match\nnext\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 5, None);
    s.toggle_mode();
    for c in "mat".chars() {
        s.push(c);
    }
    s.refresh_now();
    assert_eq!(s.content_results.len(), 1);
    assert_eq!(s.content_results[0].context.len(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_reload_files() {
    let root = search_temp_dir("reload");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    assert_eq!(s.file_results.len(), 1);

    fs::write(root.join("b.txt"), "world\n").unwrap();
    s.reload_files(&root, false, true, None);
    assert_eq!(s.file_results.len(), 2);
    fs::remove_dir_all(&root).ok();
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

// -- TreeFilter --------------------------------------------------------------

#[test]
fn tree_filter_new_has_no_cache() {
    let f = TreeFilter::new();
    assert!(f.cached.is_none());
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
