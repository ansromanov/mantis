//! Generic fold state management on `App`.
//!
//! Bridges the pure fold-region types in `crate::fold` to the live content
//! pane. It tracks which regions are collapsed (`folded`), computes the
//! fold-gutter width, maps a physical line to its fold region, and rebuilds
//! `fold_display_map` — the display-line to physical-line table the renderer
//! and scroll math consult while folds are active. Toggling a fold here keeps
//! that map and the recorded gutter click rows in sync, so mouse and keyboard
//! fold operations always agree on what is visible. Regions are computed by the
//! file loader: the YAML indentation detector (`yaml_fold`) for YAML, and the
//! per-language detectors in `crate::fold_detectors` for everything else.

use super::App;

impl App {
    /// Width of the fold gutter (2 chars: marker + space).
    ///
    /// This is always returned as 2 (rather than 0 when no fold regions are
    /// detected) so that the line-number column position doesn't shift
    /// left/right when switching between files with and without fold regions.
    pub fn fold_gutter_width(&self) -> usize {
        2
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
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::FeatureUsed {
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
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::FeatureUsed {
                feature: crate::telemetry::Feature::Fold,
            });
        self.folded = (0..self.fold_regions.len()).collect();
        self.rebuild_fold_display_map();
        self.set_content_scroll(0);
    }

    /// Expands every region.
    pub fn unfold_all(&mut self) {
        self.telemetry
            .record(crate::telemetry::TelemetryEvent::FeatureUsed {
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
}

#[cfg(test)]
#[path = "fold_test.rs"]
mod tests;
