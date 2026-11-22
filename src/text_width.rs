//! Terminal cell widths and line wrapping, in one place.
//!
//! The content pane used to measure text three different ways: the renderer
//! summed per-character widths, scroll math divided a whole-string width by the
//! pane width, and ratatui laid text out by grapheme. Those disagree for wide
//! (CJK/emoji) characters, which made the number of painted rows differ from the
//! number the scroll range assumed, so the bottom of a file could be cut off or
//! the view could drift. Everything now goes through this module: the renderer
//! splits lines with [`row_breaks`], and scroll math counts rows with
//! [`wrap_row_count`], which is defined as `row_breaks(..).len()`, so the two
//! cannot disagree.
//!
//! Wrapping prefers to break after whitespace. A word longer than the pane is
//! broken at the edge, and a character that does not fit moves to the next row
//! (a character wider than the whole pane gets a row of its own so wrapping
//! always terminates). Whitespace that would overflow the edge "hangs" past it
//! instead of forcing a break, so a row never starts with a stray space.
//!
//! Public items:
//! - [`char_width`]: cells occupied by one character.
//! - [`str_width`]: cells occupied by a string.
//! - [`row_breaks`]: the character indices where each wrapped row starts.
//! - [`wrap_row_count`]: number of rows a string needs at a given width.

use unicode_width::UnicodeWidthChar;

/// Cells occupied by `c` (zero-width and control characters count as 0).
pub fn char_width(c: char) -> usize {
    UnicodeWidthChar::width(c).unwrap_or(0)
}

/// Cells occupied by `s`, as the sum of its characters' widths.
pub fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// Character indices at which each wrapped row of `chars` begins (always starts
/// with `0`). `width == 0` disables wrapping.
pub fn row_breaks(chars: impl IntoIterator<Item = char>, width: usize) -> Vec<usize> {
    let chars: Vec<char> = chars.into_iter().collect();
    let mut breaks = vec![0];
    if width == 0 || chars.is_empty() {
        return breaks;
    }
    let mut start = 0usize; // first character of the current row
    let mut used = 0usize; // cells used by the current row
    let mut last_break: Option<usize> = None; // index just after the last whitespace
    let mut i = 0usize;
    while i < chars.len() {
        let c = chars[i];
        let w = char_width(c);
        if c.is_whitespace() {
            // Whitespace never forces a break; it may hang past the edge.
            if used + w <= width {
                used += w;
            }
            i += 1;
            last_break = Some(i);
            continue;
        }
        if used + w > width && i > start {
            let b = match last_break {
                Some(b) if b > start => b,
                _ => i,
            };
            breaks.push(b);
            start = b;
            // Everything after the break point is non-whitespace by construction.
            used = chars[b..i].iter().map(|&c| char_width(c)).sum();
            last_break = None;
            continue; // re-test the current character against the new row
        }
        used += w;
        i += 1;
    }
    breaks
}

/// Number of rows `s` occupies when wrapped to `width` cells (at least 1).
pub fn wrap_row_count(s: &str, width: usize) -> usize {
    if width == 0 || str_width(s) <= width {
        return 1;
    }
    row_breaks(s.chars(), width).len()
}

#[cfg(test)]
#[path = "text_width_test.rs"]
mod tests;
