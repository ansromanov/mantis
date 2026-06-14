//! Parses unified-diff text into aligned side-by-side rows for the split diff
//! view. The parser is theme-agnostic — it only produces structure (old/new
//! cells, line numbers, and intra-line word-emphasis ranges); colors are
//! applied by the renderer in `ui/content.rs`.

/// Below this content-pane width the side-by-side view is too cramped to be
/// useful, so the renderer falls back to the unified diff.
pub const MIN_SIDE_BY_SIDE_WIDTH: u16 = 80;

/// Kind of a single cell (one side of a side-by-side row).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellKind {
    Context,
    Added,
    Removed,
    /// No line on this side — padding opposite an unpaired add/remove.
    Empty,
}

/// One side (old or new) of a side-by-side row.
#[derive(Clone, Debug)]
pub struct Cell {
    pub kind: CellKind,
    /// 1-based source line number, or `None` for `Empty` cells.
    pub line_no: Option<usize>,
    pub text: String,
    /// Char ranges `[start, end)` within `text` that differ from the paired
    /// line, for word-level highlighting. Empty for context/unpaired lines.
    pub emphasis: Emphasis,
}

/// Char ranges `[start, end)` to highlight within a line.
pub type Emphasis = Vec<(usize, usize)>;

impl Cell {
    fn empty() -> Self {
        Cell {
            kind: CellKind::Empty,
            line_no: None,
            text: String::new(),
            emphasis: Vec::new(),
        }
    }
}

/// A side-by-side diff row.
#[derive(Clone, Debug)]
pub enum DiffRow {
    /// A hunk header (`@@ ... @@`) spanning both columns.
    Header(String),
    /// A pair of cells: `left` is the old side, `right` is the new side.
    Split { left: Cell, right: Cell },
}

/// The largest old/new line numbers present, used to size the gutters.
pub fn max_line_numbers(rows: &[DiffRow]) -> (usize, usize) {
    let mut old_max = 0;
    let mut new_max = 0;
    for row in rows {
        if let DiffRow::Split { left, right } = row {
            if let Some(n) = left.line_no {
                old_max = old_max.max(n);
            }
            if let Some(n) = right.line_no {
                new_max = new_max.max(n);
            }
        }
    }
    (old_max, new_max)
}

/// Parses unified-diff `lines` into side-by-side rows. Returns an empty `Vec`
/// when there is nothing diff-shaped to render (e.g. a placeholder message),
/// signalling the caller to keep the unified view.
pub fn parse_side_by_side(lines: &[String]) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut removed: Vec<(usize, String)> = Vec::new();
    let mut added: Vec<(usize, String)> = Vec::new();
    let mut old_no = 0usize;
    let mut new_no = 0usize;
    let mut in_hunk = false;

    for line in lines {
        if let Some(rest) = line.strip_prefix("@@") {
            flush(&mut rows, &mut removed, &mut added);
            if let Some((o, n)) = parse_hunk_header(rest) {
                old_no = o;
                new_no = n;
            }
            in_hunk = true;
            rows.push(DiffRow::Header(line.clone()));
            continue;
        }

        if !in_hunk {
            // Preamble (`diff --git`, `index`, `--- a/...`, `+++ b/...`, mode
            // lines, etc.) carries no per-line content for the split view.
            continue;
        }

        if let Some(rest) = line.strip_prefix('+') {
            added.push((new_no, rest.to_string()));
            new_no += 1;
        } else if let Some(rest) = line.strip_prefix('-') {
            removed.push((old_no, rest.to_string()));
            old_no += 1;
        } else if line.starts_with('\\') {
            // "\ No newline at end of file" — not a content line.
            continue;
        } else {
            flush(&mut rows, &mut removed, &mut added);
            let text = line.strip_prefix(' ').unwrap_or(line).to_string();
            rows.push(DiffRow::Split {
                left: Cell {
                    kind: CellKind::Context,
                    line_no: Some(old_no),
                    text: text.clone(),
                    emphasis: Vec::new(),
                },
                right: Cell {
                    kind: CellKind::Context,
                    line_no: Some(new_no),
                    text,
                    emphasis: Vec::new(),
                },
            });
            old_no += 1;
            new_no += 1;
        }
    }
    flush(&mut rows, &mut removed, &mut added);
    rows
}

/// Emits the buffered removed/added lines as paired split rows, computing
/// word-level emphasis for rows where both sides are present.
fn flush(
    rows: &mut Vec<DiffRow>,
    removed: &mut Vec<(usize, String)>,
    added: &mut Vec<(usize, String)>,
) {
    let n = removed.len().max(added.len());
    for i in 0..n {
        let mut left = match removed.get(i) {
            Some((no, t)) => Cell {
                kind: CellKind::Removed,
                line_no: Some(*no),
                text: t.clone(),
                emphasis: Vec::new(),
            },
            None => Cell::empty(),
        };
        let mut right = match added.get(i) {
            Some((no, t)) => Cell {
                kind: CellKind::Added,
                line_no: Some(*no),
                text: t.clone(),
                emphasis: Vec::new(),
            },
            None => Cell::empty(),
        };
        if left.kind == CellKind::Removed && right.kind == CellKind::Added {
            let (old_emph, new_emph) = word_diff(&left.text, &right.text);
            left.emphasis = old_emph;
            right.emphasis = new_emph;
        }
        rows.push(DiffRow::Split { left, right });
    }
    removed.clear();
    added.clear();
}

/// Parses the leading line numbers from a hunk header body (the text after
/// `@@`), e.g. ` -120,7 +118,9 @@ fn foo` → `(120, 118)`.
fn parse_hunk_header(rest: &str) -> Option<(usize, usize)> {
    let mut old_no = None;
    let mut new_no = None;
    for tok in rest.split_whitespace() {
        if let Some(body) = tok.strip_prefix('-') {
            old_no = body.split(',').next().and_then(|s| s.parse().ok());
        } else if let Some(body) = tok.strip_prefix('+') {
            new_no = body.split(',').next().and_then(|s| s.parse().ok());
        }
        if old_no.is_some() && new_no.is_some() {
            break;
        }
    }
    Some((old_no?, new_no?))
}

/// Splits a string into a sequence of tokens, where each token is a maximal run
/// of either word characters (alphanumeric or `_`) or non-word characters.
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut cur_is_word: Option<bool> = None;
    for ch in s.chars() {
        let is_word = ch.is_alphanumeric() || ch == '_';
        match cur_is_word {
            Some(w) if w == is_word => cur.push(ch),
            _ => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
                cur.push(ch);
                cur_is_word = Some(is_word);
            }
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

/// Computes word-level differences between two lines via an LCS over tokens.
/// Returns the char ranges to emphasize in `old` and in `new` respectively.
fn word_diff(old: &str, new: &str) -> (Emphasis, Emphasis) {
    let a = tokenize(old);
    let b = tokenize(new);
    let n = a.len();
    let m = b.len();

    // dp[i][j] = LCS length of a[i..] and b[j..].
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }

    let mut old_emph = Vec::new();
    let mut new_emph = Vec::new();
    let mut old_pos = 0usize;
    let mut new_pos = 0usize;
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if a[i] == b[j] {
            old_pos += a[i].chars().count();
            new_pos += b[j].chars().count();
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            let len = a[i].chars().count();
            push_range(&mut old_emph, old_pos, old_pos + len);
            old_pos += len;
            i += 1;
        } else {
            let len = b[j].chars().count();
            push_range(&mut new_emph, new_pos, new_pos + len);
            new_pos += len;
            j += 1;
        }
    }
    while i < n {
        let len = a[i].chars().count();
        push_range(&mut old_emph, old_pos, old_pos + len);
        old_pos += len;
        i += 1;
    }
    while j < m {
        let len = b[j].chars().count();
        push_range(&mut new_emph, new_pos, new_pos + len);
        new_pos += len;
        j += 1;
    }
    (old_emph, new_emph)
}

/// Appends `[start, end)` to `ranges`, merging it with the previous range when
/// they are contiguous so adjacent changed tokens render as one highlight.
fn push_range(ranges: &mut Vec<(usize, usize)>, start: usize, end: usize) {
    if start >= end {
        return;
    }
    if let Some(last) = ranges.last_mut() {
        if last.1 == start {
            last.1 = end;
            return;
        }
    }
    ranges.push((start, end));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &[&str]) -> Vec<String> {
        s.iter().map(|l| l.to_string()).collect()
    }

    #[test]
    fn empty_input_yields_no_rows() {
        assert!(parse_side_by_side(&[]).is_empty());
    }

    #[test]
    fn placeholder_message_yields_no_rows() {
        // No `@@` hunk → nothing to lay out side by side.
        assert!(parse_side_by_side(&lines(&["(no diff available)"])).is_empty());
    }

    #[test]
    fn context_line_appears_on_both_sides() {
        let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", " unchanged"]));
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], DiffRow::Header(_)));
        match &rows[1] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.kind, CellKind::Context);
                assert_eq!(right.kind, CellKind::Context);
                assert_eq!(left.text, "unchanged");
                assert_eq!(right.text, "unchanged");
                assert_eq!(left.line_no, Some(1));
                assert_eq!(right.line_no, Some(1));
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn paired_change_aligns_old_and_new() {
        let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", "-let x = 1;", "+let x = 2;"]));
        // header + 1 paired split row
        assert_eq!(rows.len(), 2);
        match &rows[1] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.kind, CellKind::Removed);
                assert_eq!(right.kind, CellKind::Added);
                assert_eq!(left.text, "let x = 1;");
                assert_eq!(right.text, "let x = 2;");
                // Only the differing "1"/"2" tokens are emphasized.
                let old_seg: String = left
                    .emphasis
                    .iter()
                    .flat_map(|&(s, e)| left.text.chars().skip(s).take(e - s))
                    .collect();
                let new_seg: String = right
                    .emphasis
                    .iter()
                    .flat_map(|&(s, e)| right.text.chars().skip(s).take(e - s))
                    .collect();
                assert_eq!(old_seg, "1");
                assert_eq!(new_seg, "2");
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn extra_addition_pads_left_with_empty() {
        let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,2 @@", "-old", "+new", "+extra"]));
        // header + 2 rows (old|new, empty|extra)
        assert_eq!(rows.len(), 3);
        match &rows[2] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.kind, CellKind::Empty);
                assert_eq!(left.line_no, None);
                assert_eq!(right.kind, CellKind::Added);
                assert_eq!(right.text, "extra");
                assert_eq!(right.line_no, Some(2));
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn extra_removal_pads_right_with_empty() {
        let rows = parse_side_by_side(&lines(&["@@ -1,2 +1,1 @@", "-a", "-b", "+c"]));
        assert_eq!(rows.len(), 3);
        match &rows[2] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.kind, CellKind::Removed);
                assert_eq!(left.text, "b");
                assert_eq!(right.kind, CellKind::Empty);
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn line_numbers_track_across_hunks() {
        let rows = parse_side_by_side(&lines(&["@@ -10,2 +20,2 @@", " ctx", "-removed", "+added"]));
        // ctx is old line 10 / new line 20; the change is old 11 / new 21.
        match &rows[1] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.line_no, Some(10));
                assert_eq!(right.line_no, Some(20));
            }
            _ => panic!("expected split row"),
        }
        match &rows[2] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.line_no, Some(11));
                assert_eq!(right.line_no, Some(21));
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn preamble_before_first_hunk_is_ignored() {
        let rows = parse_side_by_side(&lines(&[
            "diff --git a/f b/f",
            "index 111..222 100644",
            "--- a/f",
            "+++ b/f",
            "@@ -1,1 +1,1 @@",
            " ctx",
        ]));
        assert_eq!(rows.len(), 2);
        assert!(matches!(rows[0], DiffRow::Header(_)));
    }

    #[test]
    fn no_newline_marker_is_skipped() {
        let rows = parse_side_by_side(&lines(&[
            "@@ -1,1 +1,1 @@",
            "-a",
            "\\ No newline at end of file",
            "+b",
        ]));
        // The marker is dropped; old/new still pair up.
        assert_eq!(rows.len(), 2);
        match &rows[1] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.text, "a");
                assert_eq!(right.text, "b");
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn removed_content_starting_with_dashes_is_not_a_header() {
        // Inside a hunk, a removed line whose content begins with "--" must be
        // treated as content, not as a `---` file header.
        let rows = parse_side_by_side(&lines(&["@@ -1,1 +1,1 @@", "--- bullet", "+- bullet"]));
        match &rows[1] {
            DiffRow::Split { left, right } => {
                assert_eq!(left.text, "-- bullet");
                assert_eq!(right.text, "- bullet");
            }
            _ => panic!("expected split row"),
        }
    }

    #[test]
    fn max_line_numbers_reports_widest_gutters() {
        let rows = parse_side_by_side(&lines(&["@@ -8,1 +120,1 @@", " ctx"]));
        assert_eq!(max_line_numbers(&rows), (8, 120));
    }

    #[test]
    fn tokenize_splits_words_and_separators() {
        // Consecutive non-word chars (space, `=`, space) form one separator run.
        assert_eq!(tokenize("a_b = 1"), vec!["a_b", " = ", "1"]);
    }

    #[test]
    fn word_diff_emphasizes_only_changed_tokens() {
        let (old_e, new_e) = word_diff("foo bar baz", "foo qux baz");
        let old_seg: String = old_e
            .iter()
            .flat_map(|&(s, e)| "foo bar baz".chars().skip(s).take(e - s))
            .collect();
        let new_seg: String = new_e
            .iter()
            .flat_map(|&(s, e)| "foo qux baz".chars().skip(s).take(e - s))
            .collect();
        assert_eq!(old_seg, "bar");
        assert_eq!(new_seg, "qux");
    }

    #[test]
    fn word_diff_identical_lines_have_no_emphasis() {
        let (old_e, new_e) = word_diff("same line", "same line");
        assert!(old_e.is_empty());
        assert!(new_e.is_empty());
    }

    #[test]
    fn push_range_merges_contiguous() {
        let mut r = Vec::new();
        push_range(&mut r, 0, 3);
        push_range(&mut r, 3, 5);
        push_range(&mut r, 7, 9);
        assert_eq!(r, vec![(0, 5), (7, 9)]);
    }
}
