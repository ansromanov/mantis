use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};
use crate::search::InFileSearch;
use crate::theme::Theme;

const SCROLLBAR_FADE: Duration = Duration::from_millis(2000);

/// Renders the content/diff panel. Handles three modes:
/// - Diff view (styled per-line, no gutter, no selection)
/// - Markdown rendered view (styled spans from `markdown_lines`)
/// - Plain / syntax-highlighted view (with line numbers and optional selection)
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

    let sel = app.selection.as_ref().map(|s| s.normalized());
    let sel_bg = app.theme.selection_bg;

    let in_file_search = app.in_file_search.as_ref();

    // ln_width: number of columns for the fixed line-number gutter (0 = no gutter).
    // ln_lines: one Line per row containing only the line-number span.
    // content_lines: one Line per row containing only the text/code spans.
    let (ln_width, ln_lines, content_lines): (usize, Vec<Line>, Vec<Line>) = if app.is_diff {
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
    } else if app.is_markdown && !app.show_raw_markdown {
        let lines = app
            .markdown_lines
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(s) = in_file_search {
                    Line::from(apply_search_to_regions(&regions_owned, i, s, &app.theme))
                } else if let Some(((sl, sc), (el, ec))) = sel {
                    if i >= sl && i <= el {
                        let col_start = if i == sl { sc } else { 0 };
                        let col_end = if i == el { ec } else { usize::MAX };
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
        (0, vec![], lines)
    } else {
        let lw = app.content.len().to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        let num_lines = if !app.highlighted.is_empty() {
            app.highlighted.len()
        } else {
            app.content.len()
        };
        let gutters: Vec<Line> = (0..num_lines)
            .map(|i| {
                Line::from(Span::styled(
                    format!("{:>width$} ", i + 1, width = lw),
                    ln_style,
                ))
            })
            .collect();
        let content: Vec<Line> = if !app.highlighted.is_empty() {
            app.highlighted
                .iter()
                .enumerate()
                .map(|(i, regions)| {
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&regions_owned, i, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
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
            app.content
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    let region = vec![(Style::default(), text.clone())];
                    let spans: Vec<Span> = if let Some(s) = in_file_search {
                        apply_search_to_regions(&region, i, s, &app.theme)
                    } else if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
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
        // +1 for the trailing space after the digits
        (lw + 1, gutters, content)
    };

    let inner = block.inner(area);
    f.render_widget(block, area);

    let hscroll = if app.word_wrap {
        0
    } else {
        app.content_hscroll as u16
    };

    // Fixed gutter: line numbers scroll vertically but never horizontally.
    if ln_width > 0 {
        f.render_widget(
            Paragraph::new(ln_lines).scroll((app.content_scroll as u16, 0)),
            Rect {
                x: inner.x,
                y: inner.y,
                width: ln_width as u16,
                height: inner.height,
            },
        );
    }

    // Scrollable content area, offset to the right of the gutter.
    let cx = inner.x + ln_width as u16;
    let cw = inner.width.saturating_sub(ln_width as u16);
    let mut para = Paragraph::new(content_lines).scroll((app.content_scroll as u16, hscroll));
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
    let total = app.content_line_count();
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
