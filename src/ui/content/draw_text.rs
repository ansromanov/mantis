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
                        let result = app.highlight_lines(&path_buf, &lines);
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

#[cfg(test)]
#[path = "draw_text_test.rs"]
mod draw_text_tests;
