use std::collections::HashSet;

use super::{build_display_map, FoldRegion};

fn region(start: usize, end: usize) -> FoldRegion {
    FoldRegion { start, end }
}

#[test]
fn no_folds_returns_empty_map() {
    let regions = vec![region(0, 2), region(4, 6)];
    let folded = HashSet::new();
    let map = build_display_map(&regions, &folded, 8);
    assert!(
        map.is_empty(),
        "no active folds should produce an empty map"
    );
}

#[test]
fn folded_region_hides_children() {
    // Lines 0..3, region 0..2: fold 0 hides lines 1 and 2.
    let regions = vec![region(0, 2)];
    let mut folded = HashSet::new();
    folded.insert(0);
    let map = build_display_map(&regions, &folded, 4);
    // Display lines: 0 (header), 3 (after fold)
    assert_eq!(map, vec![0, 3]);
}

#[test]
fn nested_inner_fold_only() {
    // 5 lines: outer 0..3, inner 1..2
    let regions = vec![region(0, 3), region(1, 2)];
    let mut folded = HashSet::new();
    folded.insert(1); // fold only inner
    let map = build_display_map(&regions, &folded, 5);
    // Line 2 is hidden. Display: 0, 1, 3, 4
    assert_eq!(map, vec![0, 1, 3, 4]);
}

#[test]
fn empty_regions_returns_empty_map() {
    let regions: Vec<FoldRegion> = Vec::new();
    let mut folded = HashSet::new();
    folded.insert(0);
    let map = build_display_map(&regions, &folded, 5);
    assert!(map.is_empty(), "empty region list should produce no map");
}

#[test]
fn all_regions_folded() {
    // 6 lines: two separate foldable blocks
    // region 0: lines 0..1 (fold hides line 1)
    // region 1: lines 3..5 (fold hides lines 4 and 5)
    let regions = vec![region(0, 1), region(3, 5)];
    let mut folded = HashSet::new();
    folded.insert(0);
    folded.insert(1);
    let map = build_display_map(&regions, &folded, 6);
    // Visible: 0, 2 (between blocks), 3 (header of second block)
    assert_eq!(map, vec![0, 2, 3]);
}
