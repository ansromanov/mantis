use super::*;

#[test]
fn build_display_map_no_folds_returns_empty() {
    let regions = vec![FoldRegion { start: 0, end: 2 }];
    let map = build_display_map(&regions, &HashSet::new(), 3);
    assert!(map.is_empty());
}

#[test]
fn build_display_map_folded_hides_children() {
    let regions = vec![FoldRegion { start: 0, end: 2 }];
    let mut folded = HashSet::new();
    folded.insert(0);
    let map = build_display_map(&regions, &folded, 4);
    assert_eq!(map, vec![0, 3]);
}

#[test]
fn build_display_map_nested_inner_only() {
    let regions = vec![
        FoldRegion { start: 0, end: 3 },
        FoldRegion { start: 1, end: 2 },
    ];
    let mut folded = HashSet::new();
    folded.insert(1);
    let map = build_display_map(&regions, &folded, 5);
    assert_eq!(map, vec![0, 1, 3, 4]);
}

#[test]
fn build_display_map_empty_regions_returns_empty() {
    let map = build_display_map(&[], &HashSet::new(), 10);
    assert!(map.is_empty());
}

#[test]
fn build_display_map_all_folded() {
    let regions = vec![
        FoldRegion { start: 0, end: 2 },
        FoldRegion { start: 3, end: 4 },
    ];
    let mut folded = HashSet::new();
    folded.insert(0);
    folded.insert(1);
    let map = build_display_map(&regions, &folded, 5);
    assert_eq!(map, vec![0, 3]);
}
