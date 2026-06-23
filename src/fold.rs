//! Generic fold-region types and display-map computation.
//!
//! Defines [`FoldRegion`] — a (start, end) line range that can be collapsed —
//! and [`build_display_map`] which maps display lines to physical file lines
//! given a set of folded region indices. The actual detection of foldable
//! regions (indentation-based for YAML, or plugin-provided) lives elsewhere;
//! this module only provides the data model and the display-map algorithm that
//! turns a set of folded regions into a flat display→physical lookup table.

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

/// Builds a display→physical line mapping given the set of folded region indices.
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
