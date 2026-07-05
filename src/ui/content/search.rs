//! In-file search match highlighting for the content pane.
//!
//! `apply_search_to_regions` takes a line's already-styled `(Style, String)`
//! regions and the active `InFileSearch` state and subdivides those regions at
//! match boundaries, so each match on the line is recolored: the current match
//! gets the selection background and the others get a dimmer highlight. It
//! returns ratatui `Span`s ready to render. The work is per-line and purely
//! presentational - match positions are computed by the search engine; this
//! module only overlays their styling onto the existing syntax/markdown spans
//! without disturbing the underlying colors elsewhere on the line.

use ratatui::style::Style;
use ratatui::text::Span;

use crate::search::InFileSearch;
use crate::theme::Theme;

/// Subdivides styled regions at in-file search match boundaries, applying
/// `selection_bg` for the current match and `dim` for other matches.
pub(crate) fn apply_search_to_regions(
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

#[cfg(test)]
#[path = "search_test.rs"]
mod tests;
