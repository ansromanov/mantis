use super::*;

use std::path::PathBuf;

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
