use std::fs;
use std::path::PathBuf;

use super::*;
use crate::config::Config;
use crate::fold::FoldRegion;
use ratatui::layout::Rect;

fn temp_tree() -> PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_fold_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::write(dir.join("b.txt"), "hello\n").unwrap();
    let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
    fs::write(dir.join("long.txt"), long).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn set_fold_regions(app: &mut App) {
    app.fold_regions = vec![
        FoldRegion { start: 1, end: 3 },
        FoldRegion { start: 5, end: 8 },
        FoldRegion { start: 10, end: 12 },
    ];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
}

#[test]
fn fold_gutter_width_always_two_even_without_regions() {
    let root = temp_tree();
    let app = app_for(&root);
    assert_eq!(app.fold_gutter_width(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn fold_gutter_width_two_when_regions_exist() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.fold_regions = vec![FoldRegion { start: 1, end: 3 }];
    assert_eq!(app.fold_gutter_width(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn region_idx_at_finds_matching_region() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_fold_regions(&mut app);

    assert_eq!(app.region_idx_at(1), Some(0));
    assert_eq!(app.region_idx_at(5), Some(1));
    assert_eq!(app.region_idx_at(10), Some(2));

    fs::remove_dir_all(&root).ok();
}

#[test]
fn region_idx_at_returns_none_for_non_matching_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_fold_regions(&mut app);

    assert_eq!(app.region_idx_at(0), None);
    assert_eq!(app.region_idx_at(2), None);
    assert_eq!(app.region_idx_at(99), None);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_fold_region_adds_and_removes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    // Open long.txt to have enough content for the fold regions
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };
    // Set fold regions AFTER open_file (which calls clear_fold_state)
    app.fold_regions = vec![
        FoldRegion { start: 1, end: 3 },
        FoldRegion { start: 5, end: 8 },
        FoldRegion { start: 10, end: 12 },
    ];

    // Initially no regions folded
    assert!(app.folded.is_empty());

    // Toggle region 0 on
    app.toggle_fold_region(0);
    assert_eq!(app.folded.len(), 1);
    assert!(app.folded.contains(&0));
    assert!(!app.fold_display_map.is_empty());

    // Toggle region 0 off
    app.toggle_fold_region(0);
    assert!(app.folded.is_empty());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_fold_region_rebuilds_display_map() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.fold_regions = vec![FoldRegion { start: 0, end: 2 }];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };

    // Toggle the region on; display map should shrink
    app.toggle_fold_region(0);
    assert!(!app.fold_display_map.is_empty());
    let _display_count_folded = app.display_line_count();

    // Toggle off; display should go back to full
    app.toggle_fold_region(0);
    assert!(app.fold_display_map.is_empty());
    assert_eq!(app.display_line_count(), app.line_count());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_fold_region_clamps_scroll() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    app.fold_regions = vec![
        FoldRegion { start: 0, end: 45 },
        FoldRegion { start: 46, end: 48 },
    ];
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 10,
    };

    // Scroll to near the end
    app.content_scroll = 30;
    // Fold all — this brings display_line_count() down to ~3, so scroll_max = 0
    app.toggle_fold_region(0);
    app.toggle_fold_region(1);
    // scroll should be clamped to scroll_max
    assert!(app.content_scroll <= app.content_scroll_max());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn fold_all_collapses_everything_and_scrolls_to_zero() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    set_fold_regions(&mut app);

    // Move scroll away from 0
    app.content_scroll = 15;

    app.fold_all();
    assert_eq!(app.folded.len(), app.fold_regions.len());
    assert!(app.folded.contains(&0));
    assert!(app.folded.contains(&1));
    assert!(app.folded.contains(&2));
    assert_eq!(app.content_scroll, 0);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn unfold_all_clears_folded_and_display_map() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("long.txt")); // 50 lines
    set_fold_regions(&mut app);

    // Fold everything first
    app.fold_all();
    assert!(!app.folded.is_empty());
    assert!(!app.fold_display_map.is_empty());

    // Now unfold
    app.unfold_all();
    assert!(app.folded.is_empty());
    assert!(app.fold_display_map.is_empty());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn clear_fold_state_resets_all_fold_fields() {
    let root = temp_tree();
    let mut app = app_for(&root);
    set_fold_regions(&mut app);
    app.fold_regions = vec![FoldRegion { start: 1, end: 3 }];
    app.folded.insert(0);
    app.fold_display_map = vec![0, 1, 4];
    app.fold_gutter_rows = vec![(1, 0)];
    app.yaml_error = Some("syntax error".to_string());
    app.yaml_anchor_count = 5;
    app.yaml_alias_count = 3;

    app.clear_fold_state();

    assert!(app.fold_regions.is_empty());
    assert!(app.folded.is_empty());
    assert!(app.fold_display_map.is_empty());
    assert!(app.fold_gutter_rows.is_empty());
    assert_eq!(app.yaml_error, None);
    assert_eq!(app.yaml_anchor_count, 0);
    assert_eq!(app.yaml_alias_count, 0);

    fs::remove_dir_all(&root).ok();
}

#[test]
fn apply_plugin_fold_regions_replaces_builtin_regions() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let path = root.join("test.yaml");

    // Open a bigger file so fold regions have room to fold
    app.open_file(&root.join("long.txt")); // 50 lines
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 20,
    };

    // Set up built-in regions AFTER open_file (which calls clear_fold_state)
    app.fold_regions = vec![FoldRegion { start: 0, end: 5 }];

    // Set up plugin regions for this path
    let plugin_regions = vec![
        FoldRegion { start: 0, end: 2 },
        FoldRegion { start: 3, end: 5 },
    ];
    app.plugin_fold_regions
        .insert(path.clone(), plugin_regions.clone());

    // Apply plugin fold regions
    app.apply_plugin_fold_regions(&path);

    assert_eq!(app.fold_regions.len(), 2);
    assert_eq!(app.fold_regions[0].start, 0);
    assert_eq!(app.fold_regions[0].end, 2);
    assert_eq!(app.fold_regions[1].start, 3);
    assert_eq!(app.fold_regions[1].end, 5);
    assert!(app.folded.is_empty());

    fs::remove_dir_all(&root).ok();
}

#[test]
fn apply_plugin_fold_regions_noop_when_path_not_mapped() {
    let root = temp_tree();
    let mut app = app_for(&root);

    // Set up built-in regions
    app.fold_regions = vec![FoldRegion { start: 0, end: 5 }];
    let before = app.fold_regions.clone();

    // Call with a path not in plugin_fold_regions
    app.apply_plugin_fold_regions(&root.join("other.yaml"));

    // Built-in regions should be unchanged (compare fields since FoldRegion !PartialEq)
    assert_eq!(app.fold_regions.len(), before.len());
    for (a, b) in app.fold_regions.iter().zip(before.iter()) {
        assert_eq!(a.start, b.start);
        assert_eq!(a.end, b.end);
    }

    fs::remove_dir_all(&root).ok();
}

#[test]
fn fold_telemetry_check() {
    let root = temp_tree();
    let app = app_for(&root);
    assert!(!app.telemetry.is_enabled());
    fs::remove_dir_all(&root).ok();
}
