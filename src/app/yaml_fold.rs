//! YAML fold state management on `App`.
//!
//! Bridges the pure fold-region detection in the crate-level
//! [`yaml_fold`](crate::yaml_fold) module to the live content pane. It tracks
//! which regions are collapsed (`yaml_folded`), computes the fold-gutter width,
//! maps a physical line to its fold region, and rebuilds `fold_display_map` -
//! the display-line to physical-line table the renderer and scroll math consult
//! while folds are active. Toggling a fold here keeps that map and the recorded
//! gutter click rows in sync, so mouse and keyboard fold operations always
//! agree on what is visible.

use super::App;

impl App {
    /// Width of the fold gutter (2 chars: marker + space) when YAML regions
    /// are detected, 0 otherwise.
    pub fn fold_gutter_width(&self) -> usize {
        if self.yaml_fold_regions.is_empty() {
            0
        } else {
            2
        }
    }

    /// Returns the fold region index whose `start` matches `physical_line`, if any.
    pub fn region_idx_at(&self, physical_line: usize) -> Option<usize> {
        self.yaml_fold_regions
            .iter()
            .position(|r| r.start == physical_line)
    }

    /// Rebuilds `fold_display_map` from the current `yaml_folded` set.
    pub fn rebuild_fold_display_map(&mut self) {
        self.fold_display_map = crate::yaml_fold::build_display_map(
            &self.yaml_fold_regions,
            &self.yaml_folded,
            self.line_count(),
        );
    }

    /// Toggles the fold state of `region_idx` and clamps the scroll position.
    pub fn toggle_fold_region(&mut self, region_idx: usize) {
        if self.yaml_folded.contains(&region_idx) {
            self.yaml_folded.remove(&region_idx);
        } else {
            self.yaml_folded.insert(region_idx);
        }
        self.rebuild_fold_display_map();
        self.content_scroll = self.content_scroll.min(self.content_scroll_max());
    }

    /// Folds every detected YAML region and scrolls to the top.
    pub fn fold_all(&mut self) {
        self.yaml_folded = (0..self.yaml_fold_regions.len()).collect();
        self.rebuild_fold_display_map();
        self.content_scroll = 0;
    }

    /// Expands every YAML region.
    pub fn unfold_all(&mut self) {
        self.yaml_folded.clear();
        self.fold_display_map.clear();
    }

    /// Resets all YAML state. Called whenever a new file is opened.
    pub(crate) fn clear_yaml_state(&mut self) {
        self.yaml_fold_regions.clear();
        self.yaml_folded.clear();
        self.fold_display_map.clear();
        self.fold_gutter_rows.clear();
        self.yaml_error = None;
        self.yaml_anchor_count = 0;
        self.yaml_alias_count = 0;
    }
}
