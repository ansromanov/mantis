use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};

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

    app.content_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
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
