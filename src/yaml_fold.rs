//! YAML fold-region detection (pure, UI-agnostic).
//!
//! Computes which line ranges of a YAML document can be collapsed, based purely
//! on indentation nesting: a `FoldRegion` starts at a line immediately followed
//! by a more-indented line and ends just before indentation returns to the same
//! or a shallower level. `detect_fold_regions` produces these regions and
//! `build_display_map` turns a set of collapsed regions into a display-line to
//! physical-line table. This module knows nothing about `App` or rendering;
//! `app::yaml_fold` adapts it to live editor state and the UI consumes the
//! resulting map for scrolling and the fold gutter.

use std::collections::HashSet;

/// A contiguous foldable block in a YAML file.
///
/// `start` is the "header" line (always visible). `end` is the last line that
/// belongs to the block (inclusive). Folding hides lines `start+1..=end`.
#[derive(Clone, Debug)]
pub struct FoldRegion {
    pub start: usize,
    pub end: usize,
}

/// Detects foldable regions in YAML content by indentation nesting.
///
/// A foldable region begins at any non-blank line that is immediately followed
/// (ignoring blank lines) by a line with strictly greater indentation. The
/// region ends just before the next line that returns to the same or lesser
/// indentation level.
pub fn detect_fold_regions(lines: &[impl AsRef<str>]) -> Vec<FoldRegion> {
    let n = lines.len();
    if n == 0 {
        return Vec::new();
    }

    let indent: Vec<Option<usize>> = lines
        .iter()
        .map(|l| {
            let s = l.as_ref();
            if s.trim().is_empty() {
                None
            } else {
                Some(s.len() - s.trim_start().len())
            }
        })
        .collect();

    let mut regions = Vec::new();

    for i in 0..n {
        let Some(curr_indent) = indent[i] else {
            continue;
        };

        // Find the next non-blank line
        let next = (i + 1..n).find_map(|j| indent[j].map(|ind| (j, ind)));

        let Some((_, next_ind)) = next else {
            continue;
        };

        if next_ind <= curr_indent {
            continue;
        }

        // This line has children. Find the last child line.
        let mut end = i;
        for (j, ind_opt) in indent.iter().enumerate().skip(i + 1) {
            if let Some(ind) = *ind_opt {
                if ind <= curr_indent {
                    break;
                }
            }
            end = j;
        }

        if end > i {
            regions.push(FoldRegion { start: i, end });
        }
    }

    regions
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

/// Counts YAML anchors (`&name`) and aliases (`*name`) across `lines`,
/// skipping comment lines. A marker counts only when immediately followed by an
/// alphanumeric or underscore so bare `&`/`*` (e.g. in flow text) is ignored.
pub fn count_anchors_aliases(lines: &[String]) -> (usize, usize) {
    let mut anchors = 0;
    let mut aliases = 0;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        let mut chars = trimmed.chars().peekable();
        while let Some(ch) = chars.next() {
            let starts_name = chars
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_');
            if ch == '&' && starts_name {
                anchors += 1;
            }
            if ch == '*' && starts_name {
                aliases += 1;
            }
        }
    }
    (anchors, aliases)
}

#[cfg(test)]
#[path = "yaml_fold_test.rs"]
mod tests;
