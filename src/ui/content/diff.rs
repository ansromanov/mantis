//! Side-by-side diff renderer.
//!
//! `draw_side_by_side_diff` lays out a diff as two columns - old on the left,
//! new on the right - each with its own line-number gutter and separated by a
//! vertical divider. It consumes the `DiffRow`/`Cell` structure produced by the
//! crate-level `diff` parser, applies theme colors per cell kind (context,
//! added, removed, empty padding), and renders the intra-line word-emphasis
//! ranges so paired changes are highlighted. The two halves are aligned
//! row-for-row and share one scroll offset so they move together. The caller
//! falls back to the unified view when the pane is narrower than the diff
//! module's minimum width.

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Paragraph},
    Frame,
};

use crate::app::App;
use crate::diff::{Cell, CellKind, DiffRow};

use super::scrollbar::draw_content_scrollbar;

/// Renders the diff in a split old | new layout: two columns, each with its own
/// line-number gutter, separated by a vertical divider. Removed lines sit on the
/// left, added lines on the right, with paired changes word-highlighted and
/// aligned row-for-row so the two halves scroll together.
pub(crate) fn draw_side_by_side_diff(f: &mut Frame, app: &mut App, area: Rect, block: Block) {
    let inner = block.inner(area);
    f.render_widget(block, area);

    let total = app.diff_rows.len();
    let view_height = inner.height as usize;
    let scroll = app.content_scroll.min(app.content_scroll_max());
    let end = (scroll + view_height).min(total);

    let (old_max, new_max) = crate::diff::max_line_numbers(&app.diff_rows);
    let old_digits = old_max.max(1).to_string().len();
    let new_digits = new_max.max(1).to_string().len();

    // Column geometry: left | divider | right.
    let divider_w = 1u16;
    let avail = inner.width.saturating_sub(divider_w);
    let left_col = avail / 2;
    let right_col = avail - left_col;
    let left_gutter_w = (old_digits as u16 + 1).min(left_col);
    let right_gutter_w = (new_digits as u16 + 1).min(right_col);
    let left_text_w = left_col - left_gutter_w;
    let right_text_w = right_col - right_gutter_w;

    let dim = Style::default().fg(app.theme.dim);
    let accent = Style::default().fg(app.theme.accent);
    let add_style = Style::default().fg(app.theme.diff_add);
    let del_style = Style::default().fg(app.theme.diff_del);
    let emph_add = add_style.bg(app.theme.selection_bg);
    let emph_del = del_style.bg(app.theme.selection_bg);

    let make_cell = |cell: &Cell, gutter_w: usize| -> (Line<'static>, Line<'static>) {
        let gutter = match cell.line_no {
            Some(n) => Span::styled(format!("{:>w$} ", n, w = gutter_w.saturating_sub(1)), dim),
            None => Span::styled(" ".repeat(gutter_w), dim),
        };
        let (base, emph) = match cell.kind {
            CellKind::Added => (add_style, emph_add),
            CellKind::Removed => (del_style, emph_del),
            CellKind::Context => (Style::default(), Style::default()),
            CellKind::Empty => (dim, dim),
        };
        (
            Line::from(gutter),
            Line::from(emphasize(&cell.text, &cell.emphasis, base, emph)),
        )
    };

    let mut left_gutter = Vec::with_capacity(end - scroll);
    let mut left_text = Vec::with_capacity(end - scroll);
    let mut right_gutter = Vec::with_capacity(end - scroll);
    let mut right_text = Vec::with_capacity(end - scroll);
    let mut divider = Vec::with_capacity(end - scroll);

    for row in &app.diff_rows[scroll..end] {
        match row {
            DiffRow::Header(text) => {
                left_gutter.push(Line::from(Span::styled(
                    " ".repeat(left_gutter_w as usize),
                    dim,
                )));
                left_text.push(Line::from(Span::styled(text.clone(), accent)));
                right_gutter.push(Line::from(""));
                right_text.push(Line::from(""));
            }
            DiffRow::Split { left, right } => {
                let (lg, lt) = make_cell(left, left_gutter_w as usize);
                let (rg, rt) = make_cell(right, right_gutter_w as usize);
                left_gutter.push(lg);
                left_text.push(lt);
                right_gutter.push(rg);
                right_text.push(rt);
            }
        }
        divider.push(Line::from(Span::styled("│", dim)));
    }

    let hscroll = app.content_hscroll as u16;
    let x = inner.x;
    // Gutters are fixed; text columns scroll horizontally together.
    render_column(f, left_gutter, x, inner.y, left_gutter_w, inner.height, 0);
    render_column(
        f,
        left_text,
        x + left_gutter_w,
        inner.y,
        left_text_w,
        inner.height,
        hscroll,
    );
    render_column(
        f,
        divider,
        x + left_col,
        inner.y,
        divider_w,
        inner.height,
        0,
    );
    let rx = x + left_col + divider_w;
    render_column(
        f,
        right_gutter,
        rx,
        inner.y,
        right_gutter_w,
        inner.height,
        0,
    );
    render_column(
        f,
        right_text,
        rx + right_gutter_w,
        inner.y,
        right_text_w,
        inner.height,
        hscroll,
    );

    app.content_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    app.fold_gutter_rows = Vec::new();

    draw_content_scrollbar(
        f,
        app,
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2) as usize,
        area.height.saturating_sub(2) as usize,
    );
}

/// Renders one vertical strip of pre-clipped lines at the given position,
/// applying only horizontal scroll (rows are already windowed).
fn render_column(
    f: &mut Frame,
    lines: Vec<Line<'static>>,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    hscroll: u16,
) {
    if width == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(lines).scroll((0, hscroll)),
        Rect {
            x,
            y,
            width,
            height,
        },
    );
}

/// Builds styled spans for `text`, applying `emph` to the char ranges in
/// `ranges` and `base` everywhere else. Ranges are half-open `[start, end)`.
pub(crate) fn emphasize(
    text: &str,
    ranges: &[(usize, usize)],
    base: Style,
    emph: Style,
) -> Vec<Span<'static>> {
    if ranges.is_empty() {
        return vec![Span::styled(text.to_string(), base)];
    }
    let chars: Vec<char> = text.chars().collect();
    let mut spans = Vec::new();
    let mut pos = 0;
    for &(start, end) in ranges {
        let start = start.min(chars.len());
        let end = end.min(chars.len());
        if start > pos {
            spans.push(Span::styled(
                chars[pos..start].iter().collect::<String>(),
                base,
            ));
        }
        if end > start {
            spans.push(Span::styled(
                chars[start..end].iter().collect::<String>(),
                emph,
            ));
        }
        pos = end.max(pos);
    }
    if pos < chars.len() {
        spans.push(Span::styled(chars[pos..].iter().collect::<String>(), base));
    }
    spans
}
