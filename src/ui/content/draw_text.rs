//! Virtual-file and inline-fallback content rendering.
//!
//! These two branches of `draw_content` share the same structure (display_phys
//! mapping, fold markers, blame/line-number gutters) and are the longest arms
//! of the main content-render match. Extracting them here keeps `draw.rs`
//! under 700 lines.

use std::path::PathBuf;

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::search::InFileSearch;
use crate::virtual_file::VirtualFile;

use super::draw::BLAME_COL_WIDTH;
use super::search::apply_search_to_regions;
use super::selection::apply_selection;

/// Renders content from a `VirtualFile` with on-the-fly highlighting.
/// Returns (ln_width, gutter_lines, content_lines, fold_gutter_rows).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn render_virtual_file<'a>(
    app: &'a App,
    vf: &'a VirtualFile,
    inner: Rect,
    scroll: usize,
    visible_end: usize,
    blame_annotations: &'a [String],
    blame_width: usize,
    blame_style: Style,
    show_ln: bool,
    in_file_search: Option<&'a InFileSearch>,
    sel: Option<((usize, usize), (usize, usize))>,
    sel_bg: ratatui::style::Color,
) -> (usize, Vec<Line<'a>>, Vec<Line<'a>>, Vec<(u16, usize)>) {
    let phys_total = app.line_count();
    let lw = phys_total.to_string().len().max(1);
    let fold_gw = app.fold_gutter_width();
    let ln_style = Style::default().fg(app.theme.dim);
    let fold_marker_style = Style::default().fg(app.theme.dim);
    let ellipsis_style = Style::default().fg(app.theme.dim);

    let display_phys: Vec<usize> = (scroll..visible_end)
        .map(|d| app.display_to_physical(d))
        .collect();

    let highlighted = {
        let path_buf = match &app.current_file {
            Some(p) => p.clone(),
            None => PathBuf::new(),
        };
        if path_buf.as_os_str().is_empty() {
            Vec::new()
        } else {
            let lines: Vec<&str> = display_phys
                .iter()
                .filter_map(|&i| vf.line_text(i))
                .collect();
            if lines.is_empty() {
                Vec::new()
            } else {
                let cache_key = crate::app::HighlightCacheKey {
                    path: path_buf.clone(),
                    scroll,
                    visible_end,
                    theme: app.theme.syntax.clone(),
                    word_wrap: app.word_wrap,
                };
                // Check cache (immutable borrow, released before compute path).
                let cached =
                    app.content_highlight_cache
                        .borrow()
                        .as_ref()
                        .and_then(|(key, cached)| {
                            if key == &cache_key {
                                Some(cached.clone())
                            } else {
                                None
                            }
                        });
                match cached {
                    Some(spans) => spans,
                    None => {
                        let result = app.highlight_lines(&lines);
                        *app.content_highlight_cache.borrow_mut() =
                            Some((cache_key, result.clone()));
                        result
                    }
                }
            }
        }
    };
    let has_highlight = !highlighted.is_empty();

    let mut new_fold_gutter_rows: Vec<(u16, usize)> = Vec::new();

    let gutters: Vec<Line> = display_phys
        .iter()
        .enumerate()
        .map(|(offset, &phys)| {
            let fold_marker = if fold_gw > 0 {
                if let Some(ri) = app.region_idx_at(phys) {
                    let screen_y = inner.y + offset as u16;
                    new_fold_gutter_rows.push((screen_y, ri));
                    if app.folded.contains(&ri) {
                        "▶ "
                    } else {
                        "▼ "
                    }
                } else {
                    "  "
                }
            } else {
                ""
            };
            let mut spans = Vec::new();
            if blame_width > 0 {
                let annotation = blame_annotations
                    .get(phys)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                spans.push(Span::styled(annotation, blame_style));
            }
            if fold_gw > 0 {
                spans.push(Span::styled(fold_marker.to_string(), fold_marker_style));
            }
            if show_ln {
                spans.push(Span::styled(
                    format!("{:>lw$} ", phys + 1, lw = lw),
                    ln_style,
                ));
            }
            Line::from(spans)
        })
        .collect();

    let content: Vec<Line> = display_phys
        .iter()
        .enumerate()
        .map(|(offset, &physical_idx)| {
            if fold_gw > 0 {
                if let Some(ri) = app.region_idx_at(physical_idx) {
                    if app.folded.contains(&ri) {
                        let header_spans: Vec<Span> = if has_highlight {
                            if let Some(regions) = highlighted.get(offset) {
                                let regions_owned: Vec<(Style, String)> =
                                    regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                                regions_owned
                                    .iter()
                                    .map(|(s, t)| Span::styled(t.clone(), *s))
                                    .collect()
                            } else {
                                vec![Span::raw(
                                    vf.line_text(physical_idx).unwrap_or("").to_string(),
                                )]
                            }
                        } else {
                            vec![Span::raw(
                                vf.line_text(physical_idx).unwrap_or("").to_string(),
                            )]
                        };
                        let mut line_spans = header_spans;
                        line_spans.push(Span::styled("  …", ellipsis_style));
                        return Line::from(line_spans);
                    }
                }
            }

            if has_highlight {
                if let Some(regions) = highlighted.get(offset) {
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&regions_owned, physical_idx, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if physical_idx >= sl && physical_idx <= el {
                            let col_start = if physical_idx == sl { sc } else { 0 };
                            let col_end = if physical_idx == el { ec } else { usize::MAX };
                            apply_selection(&regions_owned, col_start, col_end, sel_bg)
                        } else {
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect()
                        }
                    } else {
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect()
                    };
                    return Line::from(spans);
                }
            }
            let text = vf.line_text(physical_idx).unwrap_or("");
            let region = vec![(Style::default(), text.to_string())];
            let spans: Vec<Span> = if let Some(s) = in_file_search {
                apply_search_to_regions(&region, physical_idx, s, &app.theme)
            } else if let Some(((sl, sc), (el, ec))) = sel {
                if physical_idx >= sl && physical_idx <= el {
                    let col_start = if physical_idx == sl { sc } else { 0 };
                    let col_end = if physical_idx == el { ec } else { usize::MAX };
                    apply_selection(&region, col_start, col_end, sel_bg)
                } else {
                    vec![Span::raw(text.to_string())]
                }
            } else {
                vec![Span::raw(text.to_string())]
            };
            Line::from(spans)
        })
        .collect();
    let ln_w = if show_ln { lw + 1 } else { 0 };
    (
        blame_width + fold_gw + ln_w,
        gutters,
        content,
        new_fold_gutter_rows,
    )
}

/// Renders content from the inline `content`/`highlighted` buffers (errors,
/// binaries, small files). Returns (ln_width, gutter_lines, content_lines,
/// fold_gutter_rows).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn render_inline_fallback<'a>(
    app: &'a App,
    inner: Rect,
    scroll: usize,
    visible_end: usize,
    blame_annotations: &'a [String],
    blame_width: usize,
    blame_style: Style,
    show_ln: bool,
    in_file_search: Option<&'a InFileSearch>,
    sel: Option<((usize, usize), (usize, usize))>,
    sel_bg: ratatui::style::Color,
) -> (usize, Vec<Line<'a>>, Vec<Line<'a>>, Vec<(u16, usize)>) {
    let phys_total = app.line_count();
    let fold_gw = app.fold_gutter_width();
    let lw = phys_total.to_string().len().max(1);
    let ln_style = Style::default().fg(app.theme.dim);
    let has_highlight = !app.highlighted.is_empty();

    let display_phys: Vec<usize> = (scroll..visible_end)
        .map(|d| app.display_to_physical(d))
        .collect();

    let mut inline_fold_gutter_rows: Vec<(u16, usize)> = Vec::new();

    let gutters: Vec<Line> = display_phys
        .iter()
        .enumerate()
        .map(|(offset, &phys)| {
            let fold_marker = if fold_gw > 0 {
                if let Some(ri) = app.region_idx_at(phys) {
                    let screen_y = inner.y + offset as u16;
                    inline_fold_gutter_rows.push((screen_y, ri));
                    if app.folded.contains(&ri) {
                        "▶ "
                    } else {
                        "▼ "
                    }
                } else {
                    "  "
                }
            } else {
                ""
            };
            let mut spans = Vec::new();
            if blame_width > 0 {
                let annotation = blame_annotations
                    .get(phys)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                spans.push(Span::styled(annotation, blame_style));
            }
            if fold_gw > 0 {
                spans.push(Span::styled(fold_marker.to_string(), ln_style));
            }
            if show_ln {
                spans.push(Span::styled(
                    format!("{:>lw$} ", phys + 1, lw = lw),
                    ln_style,
                ));
            }
            Line::from(spans)
        })
        .collect();

    let content: Vec<Line> = display_phys
        .iter()
        .map(|&physical_idx| {
            if fold_gw > 0 {
                if let Some(ri) = app.region_idx_at(physical_idx) {
                    if app.folded.contains(&ri) {
                        let ellipsis_style = Style::default().fg(app.theme.dim);
                        let header_spans: Vec<Span> = if has_highlight {
                            app.highlighted
                                .get(physical_idx)
                                .map(|regions| {
                                    regions
                                        .iter()
                                        .map(|(s, t)| Span::styled(t.clone(), *s))
                                        .collect()
                                })
                                .unwrap_or_else(|| {
                                    vec![Span::raw(
                                        app.content.get(physical_idx).cloned().unwrap_or_default(),
                                    )]
                                })
                        } else {
                            vec![Span::raw(
                                app.content.get(physical_idx).cloned().unwrap_or_default(),
                            )]
                        };
                        let mut line_spans = header_spans;
                        line_spans.push(Span::styled("  …", ellipsis_style));
                        return Line::from(line_spans);
                    }
                }
            }

            if has_highlight {
                if let Some(regions) = app.highlighted.get(physical_idx) {
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&regions_owned, physical_idx, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if physical_idx >= sl && physical_idx <= el {
                            let col_start = if physical_idx == sl { sc } else { 0 };
                            let col_end = if physical_idx == el { ec } else { usize::MAX };
                            apply_selection(&regions_owned, col_start, col_end, sel_bg)
                        } else {
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect()
                        }
                    } else {
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect()
                    };
                    return Line::from(spans);
                }
            }
            let text = app
                .content
                .get(physical_idx)
                .map(|s| s.as_str())
                .unwrap_or("");
            let region = vec![(Style::default(), text.to_string())];
            let spans: Vec<Span> = if let Some(s) = in_file_search {
                apply_search_to_regions(&region, physical_idx, s, &app.theme)
            } else if let Some(((sl, sc), (el, ec))) = sel {
                if physical_idx >= sl && physical_idx <= el {
                    let col_start = if physical_idx == sl { sc } else { 0 };
                    let col_end = if physical_idx == el { ec } else { usize::MAX };
                    apply_selection(&region, col_start, col_end, sel_bg)
                } else {
                    vec![Span::raw(text.to_string())]
                }
            } else {
                vec![Span::raw(text.to_string())]
            };
            Line::from(spans)
        })
        .collect();
    let ln_w = if show_ln { lw + 1 } else { 0 };
    (
        blame_width + fold_gw + ln_w,
        gutters,
        content,
        inline_fold_gutter_rows,
    )
}

// ── Word-wrap helpers ────────────────────────────────────────────────────
//
// When word-wrap is on, content must be pre-wrapped into visual rows so the
// gutter, blame, fold markers, and active-line highlight all stay aligned with
// the content — ratatui's built-in Wrap doesn't expose row counts to the gutter
// Paragraph, causing cumulative drift on each wrapped line.

/// Width of a single character in terminal columns.
fn char_width(c: char) -> usize {
    let mut buf = [0u8; 4];
    UnicodeWidthStr::width(c.encode_utf8(&mut buf))
}

/// Group consecutive `(char, Style)` pairs into styled `Span`s, merging spans
/// that share the same style.
fn chars_to_spans(chars: &[(char, Style)]) -> Vec<Span<'static>> {
    if chars.is_empty() {
        return vec![];
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut cur_style = chars[0].1;
    for &(ch, style) in chars {
        if style != cur_style && !buf.is_empty() {
            spans.push(Span::styled(std::mem::take(&mut buf), cur_style));
            cur_style = style;
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        spans.push(Span::styled(buf, cur_style));
    }
    spans
}

/// Split a flat list of `(char, Style)` pairs into visual rows, each at most
/// `max_width` terminal columns wide. Simple character-width break (no word
/// boundary preference), matching the existing `content_pos` heuristic.
fn split_at_width(chars: &[(char, Style)], max_width: usize) -> Vec<Vec<(char, Style)>> {
    if chars.is_empty() || max_width == 0 {
        return vec![chars.to_vec()];
    }
    let mut rows: Vec<Vec<(char, Style)>> = Vec::new();
    let mut line_start = 0;
    while line_start < chars.len() {
        let mut line_width = 0usize;
        let mut i = line_start;
        while i < chars.len() {
            let (ch, _) = chars[i];
            let ch_w = char_width(ch);
            if line_width + ch_w > max_width {
                break;
            }
            line_width += ch_w;
            i += 1;
        }
        if i == chars.len() {
            rows.push(chars[line_start..].to_vec());
            break;
        }
        if line_start == i {
            // Single char (e.g. a wide CJK char) doesn't fit; include it
            // anyway to avoid an infinite loop.
            rows.push(chars[line_start..=line_start].to_vec());
            line_start += 1;
        } else {
            rows.push(chars[line_start..i].to_vec());
            line_start = i;
        }
    }
    rows
}

/// Break a single styled `Line` into one or more visual rows at `max_width`.
/// Returns one `Line` per visual row; the first is unchanged if it fits.
fn break_line<'a>(line: &Line<'a>, max_width: usize) -> Vec<Line<'a>> {
    if max_width == 0 {
        return vec![line.clone()];
    }
    let total_w: usize = line.spans.iter().map(|s| s.content.as_ref().width()).sum();
    if total_w <= max_width {
        return vec![line.clone()];
    }
    let chars: Vec<(char, Style)> = line
        .spans
        .iter()
        .flat_map(|s| s.content.chars().map(|c| (c, s.style)))
        .collect();
    let char_rows = split_at_width(&chars, max_width);
    char_rows
        .into_iter()
        .map(|row| Line::from(chars_to_spans(&row)))
        .collect()
}

/// Build a blank gutter row (same widths and styles as the original, but with
/// text replaced by spaces). Used for continuation rows of wrapped lines.
fn blank_gutter<'a>(line: &Line<'a>) -> Line<'a> {
    let spans: Vec<Span<'static>> = line
        .spans
        .iter()
        .map(|s| {
            let w = s.content.as_ref().width();
            Span::styled(" ".repeat(w), s.style)
        })
        .collect();
    Line::from(spans)
}

/// Expand content and gutter lines into visual rows for word wrap.
///
/// Each logical display line that exceeds `max_width` is broken into multiple
/// visual rows. The gutter line number (and blame/fold markers) appear only on
/// the first visual row; continuation rows get a blank gutter.
///
/// Returns `(expanded_gutters, expanded_content, visual_to_display,
/// updated_fold_rows)` where `visual_to_display[i]` gives the logical display-
/// line index (within the visible window) for visual row `i`.
#[allow(clippy::type_complexity)]
pub(crate) fn wrap_content<'a>(
    content: &[Line<'a>],
    gutters: &[Line<'a>],
    max_width: usize,
    gutter_y_base: u16,
    fold_rows: &[(u16, usize)],
) -> (Vec<Line<'a>>, Vec<Line<'a>>, Vec<usize>, Vec<(u16, usize)>) {
    let mut exp_gutters: Vec<Line<'a>> = Vec::new();
    let mut exp_content: Vec<Line<'a>> = Vec::new();
    let mut visual_to_display: Vec<usize> = Vec::new();
    let mut visual_counts: Vec<usize> = Vec::new();

    for (logical_idx, (gutter, content_line)) in gutters.iter().zip(content.iter()).enumerate() {
        let visual_rows = break_line(content_line, max_width);
        let n = visual_rows.len();
        exp_gutters.push(gutter.clone());
        for _ in 1..n {
            exp_gutters.push(blank_gutter(gutter));
        }
        exp_content.extend(visual_rows);
        visual_to_display.extend(std::iter::repeat_n(logical_idx, n));
        visual_counts.push(n);
    }

    // Adjust fold-gutter row y-coordinates: each logical line's fold marker
    // moves to the first visual row of that logical line.
    let mut cum = vec![0usize; visual_counts.len() + 1];
    for i in 0..visual_counts.len() {
        cum[i + 1] = cum[i] + visual_counts[i];
    }
    let updated_fold: Vec<(u16, usize)> = fold_rows
        .iter()
        .map(|(y, ri)| {
            let logical_off = y.saturating_sub(gutter_y_base) as usize;
            let visual_off = cum.get(logical_off).copied().unwrap_or(0);
            (gutter_y_base + visual_off as u16, *ri)
        })
        .collect();

    (exp_gutters, exp_content, visual_to_display, updated_fold)
}

#[cfg(test)]
#[path = "draw_text_test.rs"]
mod draw_text_tests;
