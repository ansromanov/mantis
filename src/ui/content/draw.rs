//! Main content-pane renderer.
//!
//! `draw_content` renders the right-hand panel across its four modes: a styled
//! unified/side-by-side diff (no gutter, no selection), a rendered-markdown view
//! from precomputed spans, a memory-mapped virtual-file view that highlights
//! only the visible window on the fly, and an inline fallback for errors,
//! binaries, and small buffers. It draws the line-number and fold gutters,
//! applies word wrap, and layers in-file search and text-selection highlighting
//! plus the transient scrollbar by calling the sibling helpers. It also records
//! the content `Rect` and scroll offsets back onto `App` for mouse hit-testing.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};

use super::diff::draw_side_by_side_diff;
use super::scrollbar::draw_content_scrollbar;
use super::search::apply_search_to_regions;
use super::selection::apply_selection;

/// Renders the content/diff panel. Handles four modes:
/// - Diff view (styled per-line, no gutter, no selection)
/// - Markdown rendered view (styled spans from `markdown_lines`)
/// - Virtual file view (mmap-backed, syntax-highlighted on the fly for the visible window)
/// - Inline fallback view (for errors, binaries, small files)
pub(crate) fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Content)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };

    let mut title = if let Some(t) = &app.content_title {
        t.clone()
    } else {
        app.current_file
            .as_ref()
            .and_then(|p| p.strip_prefix(&app.root).ok())
            .map(|rel| format!(" {} ", rel.display()))
            .unwrap_or_else(|| " No file ".into())
    };
    // While a background load is in flight the previous file's content stays on
    // screen; flag it so fast loads are invisible and slow ones are explained.
    if app.loading {
        title.push_str(" loading… ");
    }

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);

    // Side-by-side diff layout takes over the whole content pane when toggled
    // on and the pane is wide enough; otherwise we fall through to unified.
    if app.is_diff
        && app.diff_side_by_side
        && !app.diff_rows.is_empty()
        && inner.width >= crate::diff::MIN_SIDE_BY_SIDE_WIDTH
    {
        draw_side_by_side_diff(f, app, area, block);
        return;
    }

    let view_height = inner.height as usize;
    let total_lines = app.display_line_count();
    let scroll = app.content_scroll.min(total_lines.saturating_sub(1));
    let visible_end = (scroll + view_height).min(total_lines);

    let sel = app.selection.as_ref().map(|s| s.normalized());
    let sel_bg = app.theme.selection_bg;
    let in_file_search = app.in_file_search.as_ref();

    // Blame annotations: one formatted string per 0-based line index.
    // BLAME_COL_WIDTH = 7 (hash) + 1 + 10 (author) + 1 + 6 (date) + 1 = 26 chars.
    const BLAME_COL_WIDTH: usize = 26;
    let blame_annotations: Vec<String> = if app.show_blame && !app.is_diff {
        if let Some(path) = &app.current_file {
            let lines = crate::git::file_blame(&app.root, path);
            if lines.is_empty() {
                Vec::new()
            } else {
                let max_line = lines.iter().map(|l| l.line_no as usize).max().unwrap_or(0);
                let mut annotations = vec![String::new(); max_line + 1];
                for bl in &lines {
                    let idx = (bl.line_no as usize).saturating_sub(1);
                    if idx < annotations.len() {
                        let author: String = bl.author.chars().take(10).collect();
                        let date: String = bl.date_relative.chars().take(6).collect();
                        annotations[idx] = format!("{} {:<10} {:<6} ", bl.short_hash, author, date);
                    }
                }
                annotations
            }
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };
    let blame_width = if blame_annotations.is_empty() {
        0
    } else {
        BLAME_COL_WIDTH
    };
    let blame_style = Style::default().fg(app.theme.dim);
    let show_ln = app.show_line_numbers;

    // ln_width, ln_lines, content_lines, fold_gutter_rows
    let (ln_width, ln_lines, mut content_lines, new_fold_gutter_rows) = if app.is_diff {
        // Diff view: iterate all highlighted lines (diffs are never large).
        let lines = app
            .highlighted
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                Line::from(if let Some(s) = in_file_search {
                    apply_search_to_regions(&regions_owned, i, s, &app.theme)
                } else {
                    regions_owned
                        .iter()
                        .map(|(s, t)| Span::styled(t.clone(), *s))
                        .collect()
                })
            })
            .collect();
        (0, vec![], lines, vec![])
    } else if app.is_json && app.show_pretty_json && !app.json_pretty_lines.is_empty() {
        // JSON pretty view: iterate only the visible window of pre-highlighted lines.
        let ln_style = Style::default().fg(app.theme.dim);
        let lw = app.line_count().to_string().len().max(1);
        let gutters: Vec<Line> = if show_ln {
            (scroll..visible_end)
                .map(|i| {
                    Line::from(Span::styled(
                        format!("{:>width$} ", i + 1, width = lw),
                        ln_style,
                    ))
                })
                .collect()
        } else {
            vec![]
        };
        let ln_w = if show_ln { lw + 1 } else { 0 };
        let lines: Vec<Line> = app.json_pretty_lines[scroll..visible_end]
            .iter()
            .enumerate()
            .map(|(offset, spans)| {
                let logical_idx = scroll + offset;
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(s) = in_file_search {
                    Line::from(apply_search_to_regions(
                        &regions_owned,
                        logical_idx,
                        s,
                        &app.theme,
                    ))
                } else if let Some(((sl, sc), (el, ec))) = sel {
                    if logical_idx >= sl && logical_idx <= el {
                        let col_start = if logical_idx == sl { sc } else { 0 };
                        let col_end = if logical_idx == el { ec } else { usize::MAX };
                        Line::from(apply_selection(&regions_owned, col_start, col_end, sel_bg))
                    } else {
                        Line::from(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect::<Vec<_>>(),
                        )
                    }
                } else {
                    Line::from(
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();
        (ln_w, gutters, lines, vec![])
    } else if app.is_markdown && !app.show_raw_markdown {
        // Markdown: iterate only the visible window of pre-rendered lines.
        let ln_style = Style::default().fg(app.theme.dim);
        let lw = app.line_count().to_string().len().max(1);
        let ln_w = if show_ln { lw + 1 } else { 0 };
        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                let mut spans = Vec::new();
                if blame_width > 0 {
                    let annotation = blame_annotations
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                    spans.push(Span::styled(annotation, blame_style));
                }
                if show_ln {
                    spans.push(Span::styled(
                        format!("{:>width$} ", i + 1, width = lw),
                        ln_style,
                    ));
                }
                Line::from(spans)
            })
            .collect();
        let lines: Vec<Line> = app.markdown_lines[scroll..visible_end]
            .iter()
            .enumerate()
            .map(|(offset, spans)| {
                let logical_idx = scroll + offset;
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(s) = in_file_search {
                    Line::from(apply_search_to_regions(
                        &regions_owned,
                        logical_idx,
                        s,
                        &app.theme,
                    ))
                } else if let Some(((sl, sc), (el, ec))) = sel {
                    if logical_idx >= sl && logical_idx <= el {
                        let col_start = if logical_idx == sl { sc } else { 0 };
                        let col_end = if logical_idx == el { ec } else { usize::MAX };
                        Line::from(apply_selection(&regions_owned, col_start, col_end, sel_bg))
                    } else {
                        Line::from(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect::<Vec<_>>(),
                        )
                    }
                } else {
                    Line::from(
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect();
        (blame_width + ln_w, gutters, lines, vec![])
    } else if let Some(vf) = app.virtual_file.as_ref() {
        // Virtual file view: lazy-loaded from mmap, highlighted on the fly.
        let phys_total = app.line_count();
        let lw = phys_total.to_string().len().max(1);
        let fold_gw = app.fold_gutter_width();
        let ln_style = Style::default().fg(app.theme.dim);
        let fold_marker_style = Style::default().fg(app.theme.dim);
        let ellipsis_style = Style::default().fg(app.theme.dim);

        // Map display indices to physical indices for the visible window.
        let display_phys: Vec<usize> = (scroll..visible_end)
            .map(|d| app.display_to_physical(d))
            .collect();

        let highlight = || -> Vec<Vec<(Style, String)>> {
            let path = match &app.current_file {
                Some(p) => p.as_path(),
                None => return Vec::new(),
            };
            let lines: Vec<&str> = display_phys
                .iter()
                .filter_map(|&i| vf.line_text(i))
                .collect();
            if lines.is_empty() {
                return Vec::new();
            }
            app.highlight_lines(path, &lines)
        };
        let highlighted = highlight();
        let has_highlight = !highlighted.is_empty();

        // Record fold gutter rows for mouse click detection.
        let mut new_fold_gutter_rows: Vec<(u16, usize)> = Vec::new();

        let gutters: Vec<Line> = display_phys
            .iter()
            .enumerate()
            .map(|(offset, &phys)| {
                // Determine fold marker for this line.
                let fold_marker = if fold_gw > 0 {
                    if let Some(ri) = app.region_idx_at(phys) {
                        let screen_y = inner.y + offset as u16;
                        new_fold_gutter_rows.push((screen_y, ri));
                        if app.yaml_folded.contains(&ri) {
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
                // If this line is a collapsed fold header, show a dimmed ellipsis.
                if fold_gw > 0 {
                    if let Some(ri) = app.region_idx_at(physical_idx) {
                        if app.yaml_folded.contains(&ri) {
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
                // Fallback: show raw text
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
    } else {
        // Inline fallback: `content` vec is the source (errors, binaries, small files).
        let phys_total = app.line_count();
        let fold_gw = app.fold_gutter_width();
        let lw = phys_total.to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        let has_highlight = !app.highlighted.is_empty();

        let display_phys: Vec<usize> = (scroll..visible_end)
            .map(|d| app.display_to_physical(d))
            .collect();

        // Track fold gutter rows for mouse hit detection.
        let mut inline_fold_gutter_rows: Vec<(u16, usize)> = Vec::new();

        let gutters: Vec<Line> = display_phys
            .iter()
            .enumerate()
            .map(|(offset, &phys)| {
                let fold_marker = if fold_gw > 0 {
                    if let Some(ri) = app.region_idx_at(phys) {
                        let screen_y = inner.y + offset as u16;
                        inline_fold_gutter_rows.push((screen_y, ri));
                        if app.yaml_folded.contains(&ri) {
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
                // Collapsed fold header: show header + ellipsis.
                if fold_gw > 0 {
                    if let Some(ri) = app.region_idx_at(physical_idx) {
                        if app.yaml_folded.contains(&ri) {
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
                                            app.content
                                                .get(physical_idx)
                                                .cloned()
                                                .unwrap_or_default(),
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
    };

    // Visual-line mode: paint the whole-line background across the selected
    // range. The j-th rendered line maps to display index `scroll + j` in every
    // non-diff branch above, and visual-line mode is never active over a diff.
    if let Some(v) = app.visual_line.as_ref() {
        let (vstart, vend) = v.range();
        for (j, line) in content_lines.iter_mut().enumerate() {
            let disp = scroll + j;
            if disp >= vstart && disp <= vend {
                for span in &mut line.spans {
                    span.style = span.style.bg(sel_bg);
                }
            }
        }
    }

    let inner = block.inner(area);
    f.render_widget(block, area);

    let hscroll = if app.word_wrap {
        0
    } else {
        app.content_hscroll as u16
    };

    // Fixed gutter: line numbers are pre-clipped to the visible range,
    // rendered with scroll=(0,0) because they are already at the right offset.
    if ln_width > 0 {
        f.render_widget(
            Paragraph::new(ln_lines).scroll((0, 0)),
            Rect {
                x: inner.x,
                y: inner.y,
                width: ln_width as u16,
                height: inner.height,
            },
        );
    }

    // Content area: only the visible window of lines is materialised, so
    // vertical scroll is 0. Horizontal scroll is still applied.
    let cx = inner.x + ln_width as u16;
    let cw = inner.width.saturating_sub(ln_width as u16);
    let mut para = Paragraph::new(content_lines).scroll((0, hscroll));
    if app.word_wrap {
        para = para.wrap(Wrap { trim: false });
    }
    f.render_widget(
        para,
        Rect {
            x: cx,
            y: inner.y,
            width: cw,
            height: inner.height,
        },
    );

    let inner_x = area.x + 1;
    let inner_y = area.y + 1;
    let inner_w = area.width.saturating_sub(2) as usize;
    let inner_h = area.height.saturating_sub(2) as usize;

    app.content_area = Rect {
        x: inner_x,
        y: inner_y,
        width: inner_w as u16,
        height: inner_h as u16,
    };
    app.fold_gutter_rows = new_fold_gutter_rows;

    draw_content_scrollbar(f, app, inner_x, inner_y, inner_w, inner_h);
}
