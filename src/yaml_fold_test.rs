use super::*;

fn lines(s: &str) -> Vec<&str> {
    s.lines().collect()
}

#[test]
fn empty_input_returns_no_regions() {
    let empty: &[&str] = &[];
    assert!(detect_fold_regions(empty).is_empty());
}

#[test]
fn flat_yaml_returns_no_regions() {
    let ls = lines("a: 1\nb: 2\nc: 3");
    assert!(detect_fold_regions(&ls).is_empty());
}

#[test]
fn simple_nested_key() {
    let ls = lines("outer:\n  inner: 1\n  other: 2\nflat: 3");
    let r = detect_fold_regions(&ls);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
}

#[test]
fn nested_regions() {
    let ls = lines("a:\n  b:\n    c: 1\n  d: 2\ne: 3");
    let r = detect_fold_regions(&ls);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 3);
    assert_eq!(r[1].start, 1);
    assert_eq!(r[1].end, 2);
}

#[test]
fn build_display_map_no_folds_returns_empty() {
    let regions = detect_fold_regions(&lines("a:\n  b: 1\n  c: 2"));
    let map = build_display_map(&regions, &HashSet::new(), 3);
    assert!(map.is_empty());
}

#[test]
fn build_display_map_folded_hides_children() {
    let ls = lines("a:\n  b: 1\n  c: 2\nd: 3");
    let regions = detect_fold_regions(&ls);
    let mut folded = HashSet::new();
    folded.insert(0);
    let map = build_display_map(&regions, &folded, 4);
    assert_eq!(map, vec![0, 3]);
}

#[test]
fn build_display_map_nested_inner_only() {
    let ls = lines("a:\n  b:\n    c: 1\n  d: 2\ne: 3");
    let regions = detect_fold_regions(&ls);
    let mut folded = HashSet::new();
    folded.insert(1);
    let map = build_display_map(&regions, &folded, 5);
    assert_eq!(map, vec![0, 1, 3, 4]);
}
