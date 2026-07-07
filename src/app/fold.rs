//! Generic fold state management on `App`.
//!
//! Bridges the pure fold-region types in `crate::fold` to the live content
//! pane. It tracks which regions are collapsed (`folded`), computes the
//! fold-gutter width, maps a physical line to its fold region, and rebuilds
//! `fold_display_map` — the display-line to physical-line table the renderer
//! and scroll math consult while folds are active. Toggling a fold here keeps
//! that map and the recorded gutter click rows in sync, so mouse and keyboard
//! fold operations always agree on what is visible. Regions can come from
//! either the built-in YAML indentation detector (`yaml_fold`) or from a
//! language provider registered via the plugin protocol; plugin-supplied
//! regions arrive via `apply_plugin_fold_regions` and override the built-in
//! output for the matching file extension.

use super::App;

impl App {
    /// Width of the fold gutter (2 chars: marker + space) when fold regions
    /// are detected, 0 otherwise.
    pub fn fold_gutter_width(&self) -> usize {
        if self.fold_regions.is_empty() {
            0
        } else {
            2
        }
    }

    /// Returns the fold region index whose `start` matches `physical_line`, if any.
    pub fn region_idx_at(&self, physical_line: usize) -> Option<usize> {
        self.fold_regions
            .iter()
            .position(|r| r.start == physical_line)
    }

    /// Rebuilds `fold_display_map` from the current `folded` set.
    pub fn rebuild_fold_display_map(&mut self) {
        self.fold_display_map =
            crate::fold::build_display_map(&self.fold_regions, &self.folded, self.line_count());
    }

    /// Toggles the fold state of `region_idx` and clamps the scroll position.
    pub fn toggle_fold_region(&mut self, region_idx: usize) {
        self.telemetry.record(crate::telemetry::TelemetryEvent::FeatureUsed {
            feature: crate::telemetry::Feature::Fold,
        });
        if self.folded.contains(&region_idx) {
            self.folded.remove(&region_idx);
        } else {
            self.folded.insert(region_idx);
        }
        self.rebuild_fold_display_map();
        self.clamp_content_scroll();
    }

    /// Folds every detected region and scrolls to the top.
    pub fn fold_all(&mut self) {
        self.telemetry.record(crate::telemetry::TelemetryEvent::FeatureUsed {
            feature: crate::telemetry::Feature::Fold,
        });
        self.folded = (0..self.fold_regions.len()).collect();
        self.rebuild_fold_display_map();
        self.set_content_scroll(0);
    }

    /// Expands every region.
    pub fn unfold_all(&mut self) {
        self.telemetry.record(crate::telemetry::TelemetryEvent::FeatureUsed {
            feature: crate::telemetry::Feature::Fold,
        });
        self.folded.clear();
        self.fold_display_map.clear();
    }

    /// Resets all fold state. Called whenever a new file is opened.
    pub(crate) fn clear_fold_state(&mut self) {
        self.fold_regions.clear();
        self.folded.clear();
        self.fold_display_map.clear();
        self.fold_gutter_rows.clear();
        self.yaml_error = None;
        self.yaml_anchor_count = 0;
        self.yaml_alias_count = 0;
    }

    /// Applies provider-supplied fold regions for `path`, if any exist, replacing
    /// the current built-in regions. No-op when no provider has registered regions
    /// for the path.
    pub(crate) fn apply_plugin_fold_regions(&mut self, path: &std::path::Path) {
        if let Some(regions) = self.plugin_fold_regions.get(path) {
            self.fold_regions = regions.clone();
            self.folded.clear();
            self.rebuild_fold_display_map();
        }
    }
}

#[cfg(test)]
#[path = "fold_test.rs"]
mod tests;
