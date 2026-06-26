use super::*;

#[test]
fn normalized_returns_ordered_pair() {
    let sel = TextSelection {
        anchor: (5, 3),
        active: (2, 7),
    };
    let (start, end) = sel.normalized();
    assert_eq!(start, (2, 7));
    assert_eq!(end, (5, 3));
}

#[test]
fn normalized_returns_same_when_already_ordered() {
    let sel = TextSelection {
        anchor: (2, 7),
        active: (5, 3),
    };
    let (start, end) = sel.normalized();
    assert_eq!(start, (2, 7));
    assert_eq!(end, (5, 3));
}

#[test]
fn normalized_equal_positions() {
    let sel = TextSelection {
        anchor: (3, 5),
        active: (3, 5),
    };
    let (start, end) = sel.normalized();
    assert_eq!(start, (3, 5));
    assert_eq!(end, (3, 5));
}

#[test]
fn is_empty_returns_true_when_equal() {
    let sel = TextSelection {
        anchor: (1, 1),
        active: (1, 1),
    };
    assert!(sel.is_empty());
}

#[test]
fn is_empty_returns_false_when_different() {
    let sel = TextSelection {
        anchor: (1, 1),
        active: (1, 2),
    };
    assert!(!sel.is_empty());
}
