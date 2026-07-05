use super::*;

use std::fs;
use std::sync::atomic::AtomicUsize;
static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn search_temp_dir(label: &str) -> PathBuf {
    let n = TEST_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("tv_search_{}_{}_{}", label, std::process::id(), n))
}

// -- fuzzy_refilter ----------------------------------------------------------

#[test]
fn fuzzy_refilter_empty_query_returns_identity() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let items = vec!["alpha", "beta", "gamma"];
    let result = fuzzy_refilter(&items, &matcher, "", |s| std::borrow::Cow::Borrowed(*s));
    assert_eq!(result, vec![0, 1, 2]);
}

#[test]
fn fuzzy_refilter_filters_non_matching_items() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let items = vec!["alpha", "beta", "gamma"];
    let result = fuzzy_refilter(&items, &matcher, "zzz", |s| std::borrow::Cow::Borrowed(*s));
    assert!(result.is_empty());
}

#[test]
fn fuzzy_refilter_returns_matching_indices() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let items = vec!["alpha", "beta", "gamma"];
    let result = fuzzy_refilter(&items, &matcher, "bet", |s| std::borrow::Cow::Borrowed(*s));
    assert_eq!(result, vec![1]);
}

#[test]
fn fuzzy_refilter_returns_all_matched_items() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let items = vec!["foobar", "baz_bar_qux", "barn"];
    let result = fuzzy_refilter(&items, &matcher, "bar", |s| std::borrow::Cow::Borrowed(*s));
    assert_eq!(
        result.len(),
        3,
        "all items matching 'bar' should be returned"
    );
    let mut sorted = result.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1, 2]);
}

#[test]
fn fuzzy_refilter_sorts_by_descending_score() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    // "beta" is an exact/prefix match for query "beta"; "alphabeta" is weaker
    let items = vec!["alphabeta", "beta"];
    let result = fuzzy_refilter(&items, &matcher, "beta", |s| std::borrow::Cow::Borrowed(*s));
    assert_eq!(result.len(), 2);
    assert_eq!(
        result[0], 1,
        "exact match 'beta' should rank before 'alphabeta'"
    );
}

#[test]
fn fuzzy_refilter_empty_items_returns_empty() {
    let matcher = fuzzy_matcher::skim::SkimMatcherV2::default();
    let items: Vec<&str> = vec![];
    let result = fuzzy_refilter(&items, &matcher, "abc", |s| std::borrow::Cow::Borrowed(*s));
    assert!(result.is_empty());
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

#[test]
fn search_state_content_case_sensitive() {
    let root = search_temp_dir("case_sensitive");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "HelloWorld\nhelloworld\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    s.toggle_mode();
    for c in "World".chars() {
        s.push(c);
    }

    // Case-insensitive by default
    s.refresh_now();
    assert_eq!(s.content_results.len(), 2);

    // Case-sensitive
    s.case_sensitive = true;
    s.refresh_now();
    assert_eq!(s.content_results.len(), 1);
    assert_eq!(s.content_results[0].line, "HelloWorld");

    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_content_regex() {
    let root = search_temp_dir("regex");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "abc123xyz\nabc456xyz\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    s.toggle_mode();

    // literal by default
    for c in "abc[0-9]+xyz".chars() {
        s.push(c);
    }
    s.refresh_now();
    assert_eq!(s.content_results.len(), 0);

    // regex enabled
    s.regex = true;
    s.refresh_now();
    assert_eq!(s.content_results.len(), 2);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn search_state_content_whole_word() {
    let root = search_temp_dir("whole_word");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.txt"), "hello world\nhelloworld\n").unwrap();

    let mut s = SearchState::new(&root, false, true, 0, None);
    s.toggle_mode();
    for c in "hello".chars() {
        s.push(c);
    }

    // substring by default
    s.refresh_now();
    assert_eq!(s.content_results.len(), 2);

    // whole word enabled
    s.whole_word = true;
    s.refresh_now();
    assert_eq!(s.content_results.len(), 1);
    assert_eq!(s.content_results[0].line, "hello world");

    fs::remove_dir_all(&root).ok();
}
