//! Main content-pane renderer.
//!
//! `draw_content` renders the right-hand panel across its four modes: a styled
//! unified/side-by-side diff (no gutter, no selection), a plugin-rendered view
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
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus};
use crate::git::BlameLine;

use super::diff::draw_side_by_side_diff;
use super::draw_text::{render_inline_fallback, render_virtual_file, wrap_content};
use super::scrollbar::draw_content_scrollbar;
use super::search::apply_search_to_regions;
use super::selection::apply_selection;

/// Width of the inline blame column (author + subject). Exposed so tests and
/// the mouse handler can reference the same size. Cap subject to make room.
pub(crate) const BLAME_COL_WIDTH: usize = 37;

/// Formats a `BlameLine` into an inline annotation string showing `{author:<10}
/// {subject}` truncated to `BLAME_COL_WIDTH`. Exposed for testing.
pub(crate) fn format_blame_annotation(bl: &BlameLine) -> String {
    let subj_len = BLAME_COL_WIDTH.saturating_sub(11);
    let author: String = bl.author.chars().take(10).collect();
    let subject: String = bl.subject.chars().take(subj_len).collect();
    format!("{:<10} {:<width$}", author, subject, width = subj_len)
}

/// Renders the content/diff panel. Handles four modes:
/// - Diff view (styled per-line, no gutter, no selection)
/// - Plugin-rendered view (styled spans)
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
    let scroll = app.content_scroll.min(app.content_scroll_max());
    let visible_end = (scroll + view_height).min(total_lines);

    // Rendered-content source from plugins.
    let render_lines: Option<&Vec<Vec<(ratatui::style::Style, String)>>> = app
        .current_file
        .as_ref()
        .and_then(|p| app.plugin_content.get(p));

    let sel = app.selection.as_ref().map(|s| s.normalized());
    let sel_bg = app.theme.selection_bg;
    let in_file_search = app.in_file_search.as_ref();

    // Blame annotations: one formatted string per 0-based line index.
    let blame_annotations: Vec<String> = if app.show_blame && app.has_text_cursor() {
        if let Some(path) = &app.current_file {
            // Plugin-provided blame data takes precedence over live git blame.
            let git_lines = crate::git::file_blame(&app.root, path);
            let lines = if git_lines.is_empty() {
                Vec::new()
            } else {
                let max_line = git_lines
                    .iter()
                    .map(|l| l.line_no as usize)
                    .max()
                    .unwrap_or(0);
                let mut annotations = vec![String::new(); max_line + 1];
                for bl in &git_lines {
                    let idx = (bl.line_no as usize).saturating_sub(1);
                    if idx < annotations.len() {
                        annotations[idx] = format_blame_annotation(bl);
                    }
                }
                annotations
            };
            lines
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
    app.blame_col_width = blame_width;
    let blame_style = Style::default().fg(app.theme.dim);
    let show_ln = app.show_line_numbers;

    // ln_width, ln_lines, content_lines, fold_gutter_rows
    let (ln_width, mut ln_lines, mut content_lines, mut new_fold_gutter_rows) = if app.is_diff {
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
                        Line::from(apply_selection(
                            &regions_owned,
                            col_start,
                            col_end,
                            sel_bg,
                            app.theme.is_monochrome(),
                        ))
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
    } else if let Some(md_lines) = render_lines {
        // Rendered content (plugin): iterate only the visible
        // window of pre-rendered lines. Line numbers are hidden for rendered
        // content since rendered-line indices don't correspond to source lines
        // (rendering collapses blank lines, strips code fences, etc.). This
        // matches line_prefix_width() which already returns 0 for rendered
        // markdown, keeping input and render math consistent.
        let blame_style_inner = blame_style;
        let gutters: Vec<Line> = (scroll..visible_end)
            .map(|i| {
                let mut spans = Vec::new();
                if blame_width > 0 {
                    let annotation = blame_annotations
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| " ".repeat(BLAME_COL_WIDTH));
                    spans.push(Span::styled(annotation, blame_style_inner));
                }
                Line::from(spans)
            })
            .collect();
        let lines: Vec<Line> = md_lines[scroll..visible_end]
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
                        Line::from(apply_selection(
                            &regions_owned,
                            col_start,
                            col_end,
                            sel_bg,
                            app.theme.is_monochrome(),
                        ))
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
        (blame_width, gutters, lines, vec![])
    } else if let Some(vf) = app.virtual_file.as_ref() {
        render_virtual_file(
            app,
            vf,
            inner,
            scroll,
            visible_end,
            &blame_annotations,
            blame_width,
            blame_style,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    } else {
        render_inline_fallback(
            app,
            inner,
            scroll,
            visible_end,
            &blame_annotations,
            blame_width,
            blame_style,
            show_ln,
            in_file_search,
            sel,
            sel_bg,
        )
    };

    // Empty state hint for first-time users: show orientation if no file open.
    if content_lines.is_empty() && app.current_file.is_none() {
        let search_key = app.keys().label_for_action("search_files");
        let open_key = app.keys().label_for_action("tree_expand");
        let help_key = app.keys().label_for_action("help");
        if !search_key.is_empty() && !open_key.is_empty() {
            let help_hint = if help_key.is_empty() {
                String::new()
            } else {
                format!(" · {help_key} for help")
            };
            content_lines.push(Line::from(Span::styled(
                format!(" Press {search_key} to search, or {open_key} to open a file{help_hint}"),
                Style::default().fg(app.theme.dim),
            )));
            // Keep ln_lines in sync with content_lines so wrap_content's zip
            // (which stops at the shorter side) doesn't drop this line when
            // word wrap is on and the line-number gutter is showing.
            if ln_width > 0 {
                ln_lines.push(Line::from(""));
            }
        }
    }

    // Word-wrap expansion: break content + gutters into visual rows so they
    // stay aligned under wrap (ratatui's Wrap can't communicate row count to
    // the gutter Paragraph, causing cumulative drift on each wrapped line).
    let visual_to_display: Vec<usize> = if app.word_wrap && ln_width > 0 {
        let cw = inner.width.saturating_sub(ln_width as u16) as usize;
        if cw > 0 {
            let (exp_gutters, exp_content, vmap, updated_fold) = wrap_content(
                &content_lines,
                &ln_lines,
                cw,
                inner.y,
                &new_fold_gutter_rows,
            );
            ln_lines = exp_gutters;
            content_lines = exp_content;
            new_fold_gutter_rows = updated_fold;
            vmap
        } else {
            (0..content_lines.len()).collect()
        }
    } else {
        (0..content_lines.len()).collect()
    };

    // Active-line highlight: full-width row background + gutter caret.
    if app.has_text_cursor() && !app.diff_sbs_active() {
        let active_bg = app.theme.active_line_bg;
        let content_w = inner.width.saturating_sub(ln_width as u16) as usize;
        // Selection takes visual precedence: when it covers the active line
        // (always true mid-drag, since the drag anchor sets the cursor), skip
        // the row background so the selection_bg stays visible. The gutter
        // caret below still paints - the gutter is never part of a selection.
        let sel_covers_active = sel.is_some_and(|(start, end)| {
            let active_physical = app.display_to_physical(app.active_line);
            // apply_selection highlights a half-open [col_start, col_end) range,
            // so an end column of 0 leaves the end line unhighlighted; exclude
            // it here too or the active-line background would be suppressed
            // on a line that has no visible selection.
            let last_covered = if end.1 == 0 {
                end.0.saturating_sub(1)
            } else {
                end.0
            };
            start != end && (start.0..=last_covered).contains(&active_physical)
        });
        for (j, line) in content_lines.iter_mut().enumerate() {
            let display_line = visual_to_display.get(j).copied().unwrap_or(0);
            if sel_covers_active || scroll + display_line != app.active_line {
                continue;
            }
            // Full-width content highlight
            for span in &mut line.spans {
                span.style = span.style.bg(active_bg);
            }
            let text_w: usize = line.spans.iter().map(|s| s.content.as_ref().width()).sum();
            if text_w < content_w {
                line.spans.push(Span::styled(
                    " ".repeat(content_w - text_w),
                    Style::default().bg(active_bg),
                ));
            }
        }
        // Gutter caret: brighten the active line's gutter foreground
        for (j, gutter) in ln_lines.iter_mut().enumerate() {
            let display_line = visual_to_display.get(j).copied().unwrap_or(0);
            if scroll + display_line == app.active_line {
                for span in &mut gutter.spans {
                    span.style = span.style.bg(active_bg).fg(app.theme.accent);
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
    // When there is no gutter (ln_width == 0) but word-wrap is on, fall back to
    // ratatui's built-in Wrap — there is no drift risk without a parallel gutter
    // paragraph, so the pre-expansion path is skipped and ratatui handles it.
    let mut para = Paragraph::new(content_lines).scroll((0, hscroll));
    if app.word_wrap && ln_width == 0 {
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

#[cfg(test)]
#[path = "draw_test.rs"]
mod draw_tests;
