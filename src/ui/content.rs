use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};
use crate::git;
use crate::search::InFileSearch;
use crate::theme::Theme;

const SCROLLBAR_FADE: Duration = Duration::from_millis(2000);

/// Renders the content/diff panel. Handles four modes:
/// - Diff view (styled per-line, no gutter, no selection)
/// - Markdown rendered view (styled spans from `markdown_lines`)
/// - Virtual file view (mmap-backed, syntax-highlighted on the fly for the visible window)
/// - Inline fallback view (for errors, binaries, small files)
pub(super) fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Content)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };

    let title = if let Some(t) = &app.content_title {
        t.clone()
    } else {
        app.current_file
            .as_ref()
            .and_then(|p| p.strip_prefix(&app.root).ok())
            .map(|rel| format!(" {} ", rel.display()))
            .unwrap_or_else(|| " No file ".into())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    let view_height = inner.height as usize;
    let total_lines = app.line_count();
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
            let lines = git::file_blame(&app.root, path);
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

    // ln_width, ln_lines, content_lines
    let (ln_width, ln_lines, content_lines): (usize, Vec<Line>, Vec<Line>) = if app.is_diff {
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
        (0, vec![], lines)
    } else if app.is_json && app.show_pretty_json && !app.json_pretty_lines.is_empty() {
        // JSON pretty view: iterate only the visible window of pre-highlighted lines.
        let ln_style = Style::default().fg(app.theme.dim);
        let lw = total_lines.to_string().len().max(1);
        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                Line::from(Span::styled(
                    format!("{:>width$} ", i + 1, width = lw),
                    ln_style,
                ))
            })
            .collect();
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
        (lw + 1, gutters, lines)
    } else if app.is_markdown && !app.show_raw_markdown {
        // Markdown: iterate only the visible window of pre-rendered lines.
        let ln_style = Style::default().fg(app.theme.dim);
        let lw = total_lines.to_string().len().max(1);
        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                let ln_span = Span::styled(format!("{:>width$} ", i + 1, width = lw), ln_style);
                if blame_width > 0 {
                    let annotation = blame_annotations
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                    Line::from(vec![Span::styled(annotation, blame_style), ln_span])
                } else {
                    Line::from(ln_span)
                }
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
        (blame_width + lw + 1, gutters, lines)
    } else if let Some(vf) = app.virtual_file.as_ref() {
        // Virtual file view: lazy-loaded from mmap, highlighted on the fly.
        let lw = total_lines.to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        let highlight = || -> Vec<Vec<(Style, String)>> {
            let path = match &app.current_file {
                Some(p) => p.as_path(),
                None => return Vec::new(),
            };
            let lines: Vec<&str> = (scroll..visible_end)
                .filter_map(|i| vf.line_text(i))
                .collect();
            if lines.is_empty() {
                return Vec::new();
            }
            app.highlight_lines(path, &lines)
        };
        let highlighted = highlight();
        let has_highlight = !highlighted.is_empty();

        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                let ln_span = Span::styled(format!("{:>width$} ", i + 1, width = lw), ln_style);
                if blame_width > 0 {
                    let annotation = blame_annotations
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                    Line::from(vec![Span::styled(annotation, blame_style), ln_span])
                } else {
                    Line::from(ln_span)
                }
            })
            .collect();
        let content: Vec<Line> = (scroll..visible_end)
            .enumerate()
            .map(|(offset, logical_idx)| {
                if has_highlight {
                    if let Some(regions) = highlighted.get(offset) {
                        let regions_owned: Vec<(Style, String)> =
                            regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                        let spans: Vec<Span> = if let Some(s) = in_file_search {
                            apply_search_to_regions(&regions_owned, logical_idx, s, &app.theme)
                        } else if let Some(((sl, sc), (el, ec))) = sel {
                            if logical_idx >= sl && logical_idx <= el {
                                let col_start = if logical_idx == sl { sc } else { 0 };
                                let col_end = if logical_idx == el { ec } else { usize::MAX };
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
                // Fallback: show raw text if highlighting failed or wasn't applied
                let text = vf.line_text(logical_idx).unwrap_or("");
                let region = vec![(Style::default(), text.to_string())];
                let spans: Vec<Span> = if let Some(s) = in_file_search {
                    apply_search_to_regions(&region, logical_idx, s, &app.theme)
                } else if let Some(((sl, sc), (el, ec))) = sel {
                    if logical_idx >= sl && logical_idx <= el {
                        let col_start = if logical_idx == sl { sc } else { 0 };
                        let col_end = if logical_idx == el { ec } else { usize::MAX };
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
        (blame_width + lw + 1, gutters, content)
    } else {
        // Inline fallback: `content` vec is the source (errors, binaries, small files).
        let lw = total_lines.to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        let has_highlight = !app.highlighted.is_empty();

        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                let ln_span = Span::styled(format!("{:>width$} ", i + 1, width = lw), ln_style);
                if blame_width > 0 {
                    let annotation = blame_annotations
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                    Line::from(vec![Span::styled(annotation, blame_style), ln_span])
                } else {
                    Line::from(ln_span)
                }
            })
            .collect();
        let content: Vec<Line> = if has_highlight {
            app.highlighted[scroll..visible_end]
                .iter()
                .enumerate()
                .map(|(offset, regions)| {
                    let logical_idx = scroll + offset;
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&regions_owned, logical_idx, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if logical_idx >= sl && logical_idx <= el {
                            let col_start = if logical_idx == sl { sc } else { 0 };
                            let col_end = if logical_idx == el { ec } else { usize::MAX };
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
                    Line::from(spans)
                })
                .collect()
        } else {
            app.content[scroll..visible_end]
                .iter()
                .enumerate()
                .map(|(offset, text)| {
                    let logical_idx = scroll + offset;
                    let region = vec![(Style::default(), text.clone())];
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&region, logical_idx, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if logical_idx >= sl && logical_idx <= el {
                            let col_start = if logical_idx == sl { sc } else { 0 };
                            let col_end = if logical_idx == el { ec } else { usize::MAX };
                            apply_selection(&region, col_start, col_end, sel_bg)
                        } else {
                            vec![Span::raw(text.clone())]
                        }
                    } else {
                        vec![Span::raw(text.clone())]
                    };
                    Line::from(spans)
                })
                .collect()
        };
        (blame_width + lw + 1, gutters, content)
    };

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

    // Transient scrollbar overlay on the right edge of the content area.
    let total = app.line_count();
    if app.show_scrollbar
        && total > inner_h
        && inner_h > 0
        && inner_w > 0
        && app.content_scrolled_at.elapsed() < SCROLLBAR_FADE
    {
        let thumb_size = 1.max(inner_h * inner_h / total);
        let scroll_range = total - inner_h;
        let track_range = inner_h - thumb_size;
        let thumb_start = ((app.content_scroll * track_range + scroll_range / 2)
            .checked_div(scroll_range)
            .unwrap_or(0))
        .min(track_range);

        let lines: Vec<Line> = (0..inner_h)
            .map(|i| {
                if i >= thumb_start && i < thumb_start + thumb_size {
                    Line::from(Span::styled("█", Style::default().fg(app.theme.dim)))
                } else {
                    Line::from(Span::raw(" "))
                }
            })
            .collect();

        f.render_widget(
            Paragraph::new(lines),
            Rect {
                x: inner_x + inner_w as u16 - 1,
                y: inner_y,
                width: 1,
                height: inner_h as u16,
            },
        );
    }
}

/// Subdivides styled regions at in-file search match boundaries, applying
/// `selection_bg` for the current match and `dim` for other matches.
fn apply_search_to_regions(
    regions: &[(Style, String)],
    line_idx: usize,
    search: &InFileSearch,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let line_matches: Vec<(usize, usize, bool)> = search
        .matches
        .iter()
        .enumerate()
        .filter(|(_, m)| m.line == line_idx)
        .map(|(gi, m)| (m.col, m.col + m.len, gi == search.current))
        .collect();

    if line_matches.is_empty() {
        return regions
            .iter()
            .map(|(s, t)| Span::styled(t.clone(), *s))
            .collect();
    }

    let mut result = Vec::new();
    let mut line_char_pos = 0;

    for (style, text) in regions {
        let chars: Vec<char> = text.chars().collect();
        let span_len = chars.len();
        let span_start = line_char_pos;
        let span_end = line_char_pos + span_len;

        let local_matches: Vec<(usize, usize, bool)> = line_matches
            .iter()
            .filter(|(ms, me, _)| *ms < span_end && *me > span_start)
            .map(|(ms, me, is_cur)| {
                let local_start = ms.saturating_sub(span_start).min(span_len);
                let local_end = me.saturating_sub(span_start).min(span_len);
                (local_start, local_end, *is_cur)
            })
            .collect();

        let mut pos = 0;
        for (local_start, local_end, is_current) in &local_matches {
            if *local_end <= pos || *local_start >= span_len {
                continue;
            }
            if *local_start > pos {
                result.push(Span::styled(
                    chars[pos..*local_start].iter().collect::<String>(),
                    *style,
                ));
            }
            if *local_end > *local_start {
                let bg = if *is_current {
                    theme.selection_bg
                } else {
                    theme.dim
                };
                result.push(Span::styled(
                    chars[*local_start..*local_end].iter().collect::<String>(),
                    style.bg(bg),
                ));
            }
            pos = *local_end;
        }
        if pos < span_len {
            result.push(Span::styled(
                chars[pos..].iter().collect::<String>(),
                *style,
            ));
        }
        line_char_pos += span_len;
    }
    result
}

/// Splits each (style, text) region into up to three segments —
/// before selection, selection-highlighted, after selection — by
/// character-offset boundaries. The selected segment gets `sel_bg`.
fn apply_selection(
    regions: &[(Style, String)],
    col_start: usize,
    col_end: usize,
    sel_bg: Color,
) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    let mut col = 0;
    for (style, text) in regions {
        let chars: Vec<char> = text.chars().collect();
        let span_len = chars.len();
        let before_end = col_start.saturating_sub(col).min(span_len);
        let hl_end = if col_end == usize::MAX {
            span_len
        } else {
            col_end.saturating_sub(col).min(span_len)
        };
        if before_end > 0 {
            result.push(Span::styled(
                chars[..before_end].iter().collect::<String>(),
                *style,
            ));
        }
        if before_end < hl_end {
            result.push(Span::styled(
                chars[before_end..hl_end].iter().collect::<String>(),
                style.bg(sel_bg),
            ));
        }
        if hl_end < span_len {
            result.push(Span::styled(
                chars[hl_end..].iter().collect::<String>(),
                *style,
            ));
        }
        col += span_len;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::InFileSearch;
    use crate::theme::Theme;

    fn default_theme() -> Theme {
        Theme::default()
    }

    fn single_region(text: &str) -> Vec<(Style, String)> {
        vec![(Style::default(), text.to_string())]
    }

    fn multi_region(parts: &[&str]) -> Vec<(Style, String)> {
        parts
            .iter()
            .map(|t| (Style::default(), t.to_string()))
            .collect()
    }

    // ── apply_selection ───────────────────────────────────────────────────────

    #[test]
    fn selection_empty_cols_returns_unmodified() {
        let regions = single_region("hello world");
        let result = apply_selection(&regions, 0, 0, Color::Red);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello world");
    }

    #[test]
    fn selection_highlights_middle_range() {
        let regions = single_region("hello world");
        let result = apply_selection(&regions, 6, 11, Color::Red);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "hello ");
        assert_eq!(result[1].content, "world");
        assert_eq!(result[1].style.bg, Some(Color::Red));
    }

    #[test]
    fn selection_highlights_start_of_region() {
        let regions = single_region("hello");
        let result = apply_selection(&regions, 0, 3, Color::Blue);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "hel");
        assert_eq!(result[0].style.bg, Some(Color::Blue));
        assert_eq!(result[1].content, "lo");
    }

    #[test]
    fn selection_col_end_usize_max_goes_to_end() {
        let regions = single_region("test");
        let result = apply_selection(&regions, 2, usize::MAX, Color::Green);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "te");
        assert_eq!(result[1].content, "st");
        assert_eq!(result[1].style.bg, Some(Color::Green));
    }

    #[test]
    fn selection_spans_multiple_regions() {
        let regions = multi_region(&["abc", "def", "ghi"]);
        let result = apply_selection(&regions, 2, 7, Color::Yellow);
        // abc|def|ghi, select indices 2..7 → "cdefg"
        let total: String = result.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(total, "abcdefghi");
        // The selected portions should be in the middle spans
        let selected: Vec<&Span> = result
            .iter()
            .filter(|s| s.style.bg == Some(Color::Yellow))
            .collect();
        let selected_text: String = selected.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(selected_text, "cdefg");
    }

    #[test]
    fn selection_covers_entire_text() {
        let regions = single_region("full");
        let result = apply_selection(&regions, 0, 4, Color::Magenta);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "full");
        assert_eq!(result[0].style.bg, Some(Color::Magenta));
    }

    #[test]
    fn selection_col_start_past_end() {
        let regions = single_region("hi");
        let result = apply_selection(&regions, 10, 20, Color::Red);
        // col_start is past the end → before_end = min(10, 2) = 2 → all unselected
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hi");
        assert_eq!(result[0].style.bg, None);
    }

    // ── apply_search_to_regions ───────────────────────────────────────────────

    fn make_search(matches: Vec<crate::search::InFileMatch>, current: usize) -> InFileSearch {
        InFileSearch {
            query: "test".to_string(),
            matches,
            current,
        }
    }

    #[test]
    fn search_no_matches_returns_unmodified() {
        let regions = single_region("hello world");
        let search = InFileSearch::new();
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello world");
    }

    #[test]
    fn search_highlights_current_match() {
        let regions = single_region("abcde");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 1,
                len: 3,
            }],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        // Should produce 3 spans: "a", "bcd" (bg = selection_bg), "e"
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "a");
        assert_eq!(result[1].content, "bcd");
        assert_eq!(result[1].style.bg, Some(default_theme().selection_bg));
        assert_eq!(result[2].content, "e");
    }

    #[test]
    fn search_non_current_match_uses_dim_bg() {
        let regions = single_region("abcde");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 1,
                len: 3,
            }],
            1, // not current (current points to a different index)
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        assert_eq!(result[1].style.bg, Some(default_theme().dim));
    }

    #[test]
    fn search_multiple_matches_on_line() {
        let regions = single_region("aa bb aa");
        let search = make_search(
            vec![
                crate::search::InFileMatch {
                    line: 0,
                    col: 0,
                    len: 2,
                },
                crate::search::InFileMatch {
                    line: 0,
                    col: 6,
                    len: 2,
                },
            ],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        let highlighted: String = result
            .iter()
            .filter(|s| s.style.bg == Some(default_theme().selection_bg))
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(highlighted, "aa");
    }

    #[test]
    fn search_skips_other_lines() {
        let regions = single_region("hello");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 1,
                col: 0,
                len: 3,
            }],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
    }

    #[test]
    fn search_match_at_start_of_region() {
        let regions = single_region("hello");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 0,
                len: 2,
            }],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "he");
        assert_eq!(result[0].style.bg, Some(default_theme().selection_bg));
        assert_eq!(result[1].content, "llo");
    }

    #[test]
    fn search_match_at_end_of_region() {
        let regions = single_region("hello");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 3,
                len: 2,
            }],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "hel");
        assert_eq!(result[1].content, "lo");
        assert_eq!(result[1].style.bg, Some(default_theme().selection_bg));
    }

    #[test]
    fn search_multi_byte_chars() {
        let regions = single_region("héllo wörld");
        let search = make_search(
            vec![crate::search::InFileMatch {
                line: 0,
                col: 4,
                len: 2,
            }],
            0,
        );
        let result = apply_search_to_regions(&regions, 0, &search, &default_theme());
        let total: String = result.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(total, "héllo wörld");
    }
}
