//! Status-bar fit/elision math and layout.
//!
//! This submodule owns everything that turns the flat list of built segments
//! (a `(Span, StatusSegment, priority)` triple) into a rendered status bar:
//! allowlist filtering, priority+position elision, left/right splitting, the
//! configured `separator` prefix, per-segment `[statusbar.colors]` overrides,
//! and the single-row / two-row composition. Keeping it separate from the
//! segment *builders* in the parent module lets the width accounting (which
//! must agree between elide and compose) live in one place, and keeps the
//! size of each file under the repo's limit.

use ratatui::text::{Line, Span};

use crate::config::StatusBarConfig;
use crate::theme::Theme;

use super::{StatusSegment, StatusSide};

/// Priority levels for status-bar segments (higher = kept when eliding).
pub(super) const P_META: u8 = 1; // plugin/status messages
pub(super) const P_INFO: u8 = 2; // fold stats, badges, scroll %, file encoding
pub(super) const P_GIT: u8 = 3; // git branch info
pub(super) const P_ERR: u8 = 4; // error indicators
pub(super) const P_VER: u8 = 5; // version string

/// (span, segment, priority) triples produced by segment construction and
/// threaded through elision, splitting, and styling.
type Seg = (Span<'static>, StatusSegment, u8);

/// From a list of `(Span, StatusSegment, priority)` pairs, return a `Line`
/// with segments split into left-aligned and right-aligned groups per config.
/// Higher-priority items are kept first; within the same priority level,
/// rightmost items are dropped first — across both groups.  The right group
/// is right-anchored as a block, with padding spaces in between. Segments are
/// joined (including the leading edge of each group) with the configured
/// `separator`.
///
/// In explicit allowlist mode (either `left` or `right` is `Some`), segments
/// not listed in either list are filtered out before any width/elision math.
pub(super) fn fit_two_sided(
    segs: Vec<Seg>,
    max_width: usize,
    config: &StatusBarConfig,
    theme: &Theme,
) -> Line<'static> {
    if segs.is_empty() || max_width == 0 {
        return Line::from(Vec::<Span>::new());
    }
    let segs = filter_allowlist(segs, config);
    // After filtering, might be empty.
    if segs.is_empty() {
        return Line::from(Vec::<Span>::new());
    }
    let elided = elide(segs, max_width, &config.separator);
    let (left, right) = split_sides(elided, config);
    let (left, right) = apply_color_overrides(left, right, config, theme);
    compose_left_right(left, right, max_width, &config.separator)
}

/// Two-row variant of [`fit_two_sided`]: the left-aligned group renders on the
/// top row, the right-aligned group on the bottom row. Each row elides
/// independently to `max_width`, so on a narrow terminal segments spread across
/// the second row instead of being dropped outright; elision priorities still
/// apply per row.
pub(super) fn fit_two_row(
    segs: Vec<Seg>,
    max_width: usize,
    config: &StatusBarConfig,
    theme: &Theme,
) -> (Line<'static>, Line<'static>) {
    if segs.is_empty() || max_width == 0 {
        let empty = Line::from(Vec::<Span>::new());
        return (empty.clone(), empty);
    }
    let segs = filter_allowlist(segs, config);
    let (left, right) = split_sides(segs, config);
    let left = elide(left, max_width, &config.separator);
    let right = elide(right, max_width, &config.separator);
    let (left, right) = apply_color_overrides(left, right, config, theme);
    let top = compose_left_right(left, Vec::new(), max_width, &config.separator);
    let bottom = compose_left_right(Vec::new(), right, max_width, &config.separator);
    (top, bottom)
}

/// Applies `[statusbar]` allowlist filtering: in explicit mode (either `left`
/// or `right` is `Some`), drops segments whose id appears in neither list.
fn filter_allowlist(segs: Vec<Seg>, config: &StatusBarConfig) -> Vec<Seg> {
    if config.left.is_none() && config.right.is_none() {
        return segs;
    }
    let left_ids = config.left.as_deref().unwrap_or(&[]);
    let right_ids = config.right.as_deref().unwrap_or(&[]);
    segs.into_iter()
        .filter(|(_, id, _)| {
            let name = id.id_str();
            left_ids.iter().any(|s| s == name) || right_ids.iter().any(|s| s == name)
        })
        .collect()
}

/// Drops the lowest-priority segments until the total width fits `max_width`.
/// Segments are removed lowest priority first; among equal priorities the
/// rightmost position goes first. Widths account for the configured
/// `separator` (which replaces each segment's leading space), so wider
/// separators widen the budgeted space too. Returns the surviving segments
/// unchanged when everything already fits.
fn elide(segs: Vec<Seg>, max_width: usize, separator: &str) -> Vec<Seg> {
    if segs.is_empty() || max_width == 0 {
        return Vec::new();
    }
    let total: usize = segs
        .iter()
        .map(|(s, _, _)| segment_width_with_separator(s, separator))
        .sum();
    if total <= max_width {
        return segs;
    }

    let n = segs.len();
    let mut keep = vec![true; n];
    // Indices sorted by priority (ascending) then position (descending),
    // so we remove lowest-priority, rightmost items first.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| segs[a].2.cmp(&segs[b].2).then(b.cmp(&a)));

    let mut current_width = total;
    for idx in order {
        if current_width <= max_width {
            break;
        }
        if keep[idx] {
            current_width -= segment_width_with_separator(&segs[idx].0, separator);
            keep[idx] = false;
        }
    }

    segs.into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, t)| t)
        .collect()
}

/// Rendered width of one segment when the configured `separator` is drawn
/// ahead of it: the segment's own width minus its single leading space (which
/// the separator consumes) plus the separator's width.
fn segment_width_with_separator(span: &Span<'static>, separator: &str) -> usize {
    let has_leading_space = span.content.as_ref().starts_with(' ');
    let body = span.width().saturating_sub(usize::from(has_leading_space));
    body + Span::raw(separator.to_string()).width()
}

/// Resolves `[statusbar.colors]` overrides onto their segments: each configured
/// segment's foreground is replaced by the resolved theme color role. The
/// override replaces only the foreground, preserving the segment's modifiers
/// and the bar background. Unknown roles are ignored here — config validation
/// already reported them.
fn apply_color_overrides(
    mut left: Vec<Seg>,
    mut right: Vec<Seg>,
    config: &StatusBarConfig,
    theme: &Theme,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let recolor = |triples: &mut [Seg]| {
        for (span, id, _) in triples.iter_mut() {
            let Some(role) = config.colors.get(id.id_str()) else {
                continue;
            };
            let Some(color) = theme.role_color(role) else {
                continue;
            };
            span.style = span.style.fg(color);
        }
    };
    recolor(&mut left);
    recolor(&mut right);
    (
        left.into_iter().map(|(s, _, _)| s).collect(),
        right.into_iter().map(|(s, _, _)| s).collect(),
    )
}

/// Split kept segments into left and right groups by config.
///
/// In explicit allowlist mode (either `left` or `right` is `Some`), the
/// groups are built by iterating each list in config order, preserving the
/// user-specified sequence. In default mode (both `None`), the order is the
/// build order, partitioned by `StatusSegment::side()`.
pub(super) fn split_sides(segs: Vec<Seg>, config: &StatusBarConfig) -> (Vec<Seg>, Vec<Seg>) {
    let explicit = config.left.is_some() || config.right.is_some();
    if explicit {
        let left_ids = config.left.as_deref().unwrap_or(&[]);
        let right_ids = config.right.as_deref().unwrap_or(&[]);

        let mut left = Vec::new();
        let mut right = Vec::new();

        // Left side: iterate config order, pull matching built segment.
        for id in left_ids {
            if let Some(found) = segs.iter().find(|(_, sid, _)| sid.id_str() == id.as_str()) {
                left.push(found.clone());
            }
        }
        // Right side: iterate config order, pull matching built segment.
        for id in right_ids {
            if let Some(found) = segs.iter().find(|(_, sid, _)| sid.id_str() == id.as_str()) {
                right.push(found.clone());
            }
        }
        (left, right)
    } else {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (span, id, prio) in segs {
            if id.side() == StatusSide::Right {
                right.push((span, id, prio));
            } else {
                left.push((span, id, prio));
            }
        }
        (left, right)
    }
}

/// Compose left and right span groups into a single Line, with a padding
/// gap between them.  Right group is right-anchored flush to max_width.
/// Every segment (including the first of each group) is prefixed with the
/// configured `separator`; the segment's own historical leading space is
/// consumed by that prefix, so the default `" "` separator reproduces the
/// old bytes exactly.
pub(super) fn compose_left_right(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    max_width: usize,
    separator: &str,
) -> Line<'static> {
    let left = assemble_segments(left, separator);
    let right = assemble_segments(right, separator);
    let left_w: usize = left.iter().map(Span::width).sum();
    let right_w: usize = right.iter().map(Span::width).sum();
    let gap = max_width.saturating_sub(left_w + right_w);

    let mut all: Vec<Span<'static>> = Vec::with_capacity(left.len() + 1 + right.len());
    all.extend(left);
    if gap > 0 {
        all.push(Span::raw(" ".repeat(gap)));
    }
    all.extend(right);
    Line::from(all)
}

/// Strips a single leading space from each segment (the historical separator
/// baked into segment text) and prefixes it with the configured separator
/// instead. The separator inherits the following segment's style so the
/// default single space is styled identically to how the baked-in space was.
fn assemble_segments(spans: Vec<Span<'static>>, separator: &str) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::with_capacity(spans.len() * 2);
    for mut span in spans {
        let content = span.content.to_string();
        let stripped = content.strip_prefix(' ').unwrap_or(&content);
        if !separator.is_empty() {
            out.push(Span::styled(separator.to_string(), span.style));
        }
        span.content = std::borrow::Cow::Owned(stripped.to_string());
        out.push(span);
    }
    out
}

#[cfg(test)]
#[path = "fit_test.rs"]
mod tests;
