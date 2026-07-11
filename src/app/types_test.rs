use super::*;

// -- DiffMode -----------------------------------------------------------------

#[test]
fn diff_mode_next_cycles_all_staged_unstaged() {
    assert_eq!(DiffMode::All.next(), DiffMode::Staged);
    assert_eq!(DiffMode::Staged.next(), DiffMode::Unstaged);
    assert_eq!(DiffMode::Unstaged.next(), DiffMode::All);
}

#[test]
fn diff_mode_labels_are_distinct() {
    let labels = [
        DiffMode::All.label(),
        DiffMode::Staged.label(),
        DiffMode::Unstaged.label(),
    ];
    let unique: std::collections::HashSet<_> = labels.iter().collect();
    assert_eq!(
        unique.len(),
        3,
        "each DiffMode variant must have a unique label"
    );
}

#[test]
fn diff_mode_default_is_all() {
    assert_eq!(DiffMode::default(), DiffMode::All);
}

// -- FileAtRevision / SavedDiffState ------------------------------------------

#[test]
fn file_at_revision_stores_short_hash_and_saved_diff() {
    let saved = SavedDiffState {
        content: vec!["diff line".into()],
        highlighted: vec![],
        diff_rows: vec![],
        content_title: "diff title".into(),
        content_scroll: 3,
        active_line: 1,
        side_by_side: false,
    };
    let far = FileAtRevision {
        short: "abc1234".into(),
        saved_diff: Some(saved),
    };
    assert_eq!(far.short, "abc1234");
    assert!(far.saved_diff.is_some());
    let s = far.saved_diff.unwrap();
    assert_eq!(s.content, vec!["diff line"]);
    assert_eq!(s.content_scroll, 3);
    assert_eq!(s.active_line, 1);
    assert!(!s.side_by_side);
}

#[test]
fn file_at_revision_clone_preserves_data() {
    let far = FileAtRevision {
        short: "deadbee".into(),
        saved_diff: None,
    };
    let far2 = far.clone();
    assert_eq!(far2.short, "deadbee");
    assert!(far2.saved_diff.is_none());
}

#[test]
fn saved_diff_state_clone_preserves_all_fields() {
    let s = SavedDiffState {
        content: vec!["a".into(), "b".into()],
        highlighted: vec![],
        diff_rows: vec![],
        content_title: "title".into(),
        content_scroll: 5,
        active_line: 2,
        side_by_side: true,
    };
    let s2 = s.clone();
    assert_eq!(s2.content, vec!["a", "b"]);
    assert_eq!(s2.content_title, "title");
    assert!(s2.side_by_side);
}
