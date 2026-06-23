//! Generic fold-region types and display-map computation.
//!
//! This module provides the data model and pure computation for code folding
//! across all file types, not just YAML. A `FoldRegion` describes one
//! collapsible block; `build_display_map` converts the set of currently-folded
//! regions into a display-line-to-physical-line table that the renderer and
//! scroll math consume. Both types are file-format agnostic: the YAML
//! indentation detector in `crate::yaml_fold` and language providers registered
//! via the plugin protocol both produce `Vec<FoldRegion>` and use the same map.
//! Nothing here knows about `App` or rendering; `app::fold` adapts these types
//! to live editor state.

use std::collections::HashSet;

/// A contiguous foldable block in a file.
///
/// `start` is the "header" line (always visible). `end` is the last line that
/// belongs to the block (inclusive). Folding hides lines `start+1..=end`.
#[derive(Clone, Debug)]
pub struct FoldRegion {
    pub start: usize,
    pub end: usize,
}

/// Builds a display→physical line mapping given the folded region set.
///
/// The returned `Vec<usize>` has one entry per display line; each entry is the
/// corresponding physical (file) line index. When the returned vec is empty
/// (no regions are folded), callers should treat physical == display.
pub fn build_display_map(
    regions: &[FoldRegion],
    folded: &HashSet<usize>,
    total: usize,
) -> Vec<usize> {
    if folded.is_empty() || regions.is_empty() || total == 0 {
        return Vec::new();
    }

    // Mark every line that is hidden (inside a folded region, not the header).
    let mut hidden = vec![false; total];
    for (ri, region) in regions.iter().enumerate() {
        if folded.contains(&ri) {
            let hide_start = (region.start + 1).min(total);
            let hide_end = region.end.min(total - 1);
            for h in hidden.iter_mut().take(hide_end + 1).skip(hide_start) {
                *h = true;
            }
        }
    }

    (0..total).filter(|&i| !hidden[i]).collect()
}

#[cfg(test)]
#[path = "fold_test.rs"]
mod tests;
