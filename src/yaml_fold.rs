//! YAML fold-region detection (pure, UI-agnostic).
//!
//! Computes which line ranges of a YAML document can be collapsed, based purely
//! on indentation nesting. A foldable region starts at a line immediately
//! followed by a more-indented line and ends just before indentation returns to
//! the same or shallower level. `detect_fold_regions` produces these regions as
//! `crate::fold::FoldRegion` values; `count_anchors_aliases` scans for YAML
//! anchor/alias markers. `crate::fold_detectors::yaml_fold` delegates to
//! `detect_fold_regions` so the bundled `yaml` plugin (`plugins/yaml`) links
//! the same pure function, matching the `brace_fold`/`indent_fold` pattern used
//! by the `rust`/`go`/`python` plugins, without duplicating this module into
//! the `mantis` binary crate's separate module tree (`src/main.rs`). This
//! module knows nothing about `App` or rendering; `app::fold` adapts the
//! generic region type to live editor state. The display-map computation has
//! moved to `crate::fold::build_display_map` so all file types share the same
//! folding logic.

use crate::fold::FoldRegion;

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
