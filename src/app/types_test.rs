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
