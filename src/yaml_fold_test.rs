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
fn multiple_top_level_keys_each_get_a_region() {
    // Two sibling top-level keys, each with their own nested block, should
    // produce two independent regions rather than merging into one. This is
    // the same shape the bundled `yaml` plugin (plugins/yaml) exercises via
    // `crate::fold_detectors::yaml_fold`, which delegates to this function.
    let ls = lines("a:\n  x: 1\n  y: 2\nb:\n  z: 3");
    let r = detect_fold_regions(&ls);
    assert_eq!(r.len(), 2);
    assert_eq!(r[0].start, 0);
    assert_eq!(r[0].end, 2);
    assert_eq!(r[1].start, 3);
    assert_eq!(r[1].end, 4);
}
