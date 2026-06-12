use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};

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

    let lines: Vec<Line> = if app.is_diff {
        // Diff view: styled spans, no line-number gutter, no selection.
        app.highlighted
            .iter()
            .map(|spans| {
                Line::from(
                    spans
                        .iter()
                        .map(|(s, t)| Span::styled(t.clone(), *s))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    } else if app.is_markdown && !app.show_raw_markdown {
        app.markdown_lines
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(((sl, sc), (el, ec))) = sel {
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
            .collect()
    } else {
        let ln_width = app.content.len().to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        if !app.highlighted.is_empty() {
            app.highlighted
                .iter()
                .enumerate()
                .map(|(i, regions)| {
                    let mut spans = vec![Span::styled(
                        format!("{:>width$} ", i + 1, width = ln_width),
                        ln_style,
                    )];
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
                            spans.extend(apply_selection(
                                &regions_owned,
                                col_start,
                                col_end,
                                sel_bg,
                            ));
                        } else {
                            spans.extend(
                                regions_owned
                                    .iter()
                                    .map(|(s, t)| Span::styled(t.clone(), *s)),
                            );
                        }
                    } else {
                        spans.extend(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s)),
                        );
                    }
                    Line::from(spans)
                })
                .collect()
        } else {
            app.content
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    let mut spans = vec![Span::styled(
                        format!("{:>width$} ", i + 1, width = ln_width),
                        ln_style,
                    )];
                    if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
                            let region = vec![(Style::default(), text.clone())];
                            spans.extend(apply_selection(&region, col_start, col_end, sel_bg));
                        } else {
                            spans.push(Span::raw(text.clone()));
                        }
                    } else {
                        spans.push(Span::raw(text.clone()));
                    }
                    Line::from(spans)
                })
                .collect()
        }
    };

    let hscroll = if app.word_wrap {
        0
    } else {
        app.content_hscroll as u16
    };
    let mut para = Paragraph::new(lines)
        .block(block)
        .scroll((app.content_scroll as u16, hscroll));
    if app.word_wrap {
        para = para.wrap(Wrap { trim: false });
    }

    f.render_widget(para, area);

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
    if total > inner_h
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
