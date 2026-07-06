//! Text-selection highlighting for the content pane.
//!
//! `apply_selection` overlays a character-range selection onto a line's styled
//! regions. For each `(Style, String)` region it splits the text into up to
//! three segments - before the selection, the selected span, and after - by
//! character-offset boundaries, applying the selection background color to the
//! middle segment only. A `col_end` of `usize::MAX` selects to end of line. The
//! result is a flat list of ratatui `Span`s. Like its in-file-search sibling it
//! is purely presentational and preserves the original foreground styling of the
//! highlighted text.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// Splits each (style, text) region into up to three segments —
/// before selection, selection-highlighted, after selection — by
/// character-offset boundaries. The selected segment gets `sel_bg`.
pub(crate) fn apply_selection(
    regions: &[(Style, String)],
    col_start: usize,
    col_end: usize,
    sel_bg: Color,
    is_monochrome: bool,
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
            let mut s = *style;
            if is_monochrome {
                s = s.add_modifier(ratatui::style::Modifier::REVERSED);
            } else {
                s = s.bg(sel_bg);
            }
            result.push(Span::styled(
                chars[before_end..hl_end].iter().collect::<String>(),
                s,
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
#[path = "selection_test.rs"]
mod tests;
