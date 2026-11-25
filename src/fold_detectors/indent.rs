//! Indentation-based detectors: Python-style blocks and INI/TOML sections.

use crate::fold::FoldRegion;

const PY_CONTINUATIONS: &[&str] = &["else", "elif", "except", "finally"];

const PY_HEADERS: &[&str] = &[
    "def", "class", "if", "for", "while", "with", "try", "match", "case",
];

/// Returns `true` when `line` is a comment (`#…`) after stripping leading
/// whitespace. Comments carry no indentation significance in Python, so they
/// must not terminate a fold region even when dedented to (or past) the
/// enclosing header's indent.
fn is_comment_line(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// Returns `true` when `line` starts with a continuation keyword (`else`,
/// `elif`, `except`, `finally`) after stripping leading whitespace.
fn is_py_continuation(line: &str) -> bool {
    let trimmed = line.trim_start();
    let word_end = trimmed
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    let first = &trimmed[..word_end];
    PY_CONTINUATIONS.contains(&first)
}

/// Returns `true` when `line` starts with a compound-statement header keyword.
///
/// Recognised: `def`, `class`, `if`, `for`, `while`, `with`, `try`, `match`,
/// `case`, and `@decorator` lines.  The `async` prefix is handled so that
/// `async def`/`async for`/`async with` are also headers.
fn is_py_header(line: &str) -> bool {
    let trimmed = line.trim_start();
    let word_end = trimmed
        .find(|c: char| !c.is_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    let first = &trimmed[..word_end];

    // `async def`/`async for`/`async with` — treat as headers.
    if first == "async" {
        let rest = trimmed[word_end..].trim_start();
        let next_end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        let second = &rest[..next_end];
        return matches!(second, "def" | "for" | "with");
    }

    PY_HEADERS.contains(&first)
}

/// Detects foldable regions in Python-style indentation-based files.
///
/// A region runs from each compound-statement header (see `is_py_header`) to
/// the last line that is more indented.  Continuation keywords (`else`,
/// `elif`, `except`, `finally`) do **not** start new regions — they are
/// considered part of the preceding statement.  Blank lines are transparent
/// and do not terminate a region.
pub fn indent_fold(text: &str) -> Vec<FoldRegion> {
    let lines: Vec<&str> = text.lines().collect();
    let n = lines.len();
    if n == 0 {
        return Vec::new();
    }

    // Per-line leading whitespace count.  Blank lines → None.
    let indent: Vec<Option<usize>> = lines
        .iter()
        .map(|l| {
            let trimmed = l.trim_start();
            if trimmed.is_empty() {
                None
            } else {
                Some(l.len() - trimmed.len())
            }
        })
        .collect();

    let mut regions = Vec::new();

    for i in 0..n {
        let Some(curr_indent) = indent[i] else {
            continue;
        };

        // Only compound-statement headers initiate fold regions.
        if !is_py_header(lines[i]) {
            continue;
        }

        // Walk forward: the region extends through every line that is blank,
        // a continuation at the header's level, or strictly more-indented.
        let mut end = i;
        let mut j = i + 1;
        while j < n {
            match indent[j] {
                None => {
                    // Blank line — does not terminate the region.
                    end = j;
                    j += 1;
                }
                Some(ind) if ind > curr_indent => {
                    // More deeply indented → still inside the block.
                    end = j;
                    j += 1;
                }
                Some(_) if is_py_continuation(lines[j]) => {
                    // Continuation at same/lesser indent — pass through.
                    end = j;
                    j += 1;
                }
                Some(_) if is_comment_line(lines[j]) => {
                    // Comments have no indentation significance — pass through
                    // regardless of their column.
                    end = j;
                    j += 1;
                }
                Some(_) => {
                    // Non-blank, non-continuation at same or lesser indent
                    // terminates the block.
                    break;
                }
            }
        }

        if end > i {
            regions.push(FoldRegion { start: i, end });
        }
    }

    regions
}

/// Detects foldable INI and TOML table sections.
///
/// A section starts at a valid table header (`[table]`) or array-of-tables
/// header (`[[table]]`) and extends through the last non-blank, non-comment
/// line before the next header.
/// Headers with trailing comments are accepted, while bracket-like text in
/// values or comments is ignored because only the trimmed start of a line is
/// considered. Single-line and empty sections are omitted.
pub fn section_fold(text: &str) -> Vec<FoldRegion> {
    let lines: Vec<&str> = text.lines().collect();
    let headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(line, text)| is_toml_header(text).then_some(line))
        .collect();

    headers
        .iter()
        .enumerate()
        .filter_map(|(index, &start)| {
            let boundary = headers
                .get(index + 1)
                .copied()
                .unwrap_or(lines.len())
                .saturating_sub(1);
            let end = (start + 1..=boundary)
                .rev()
                .find(|line| !is_ignorable_section_line(lines[*line]))
                .unwrap_or(start);
            (end > start).then_some(FoldRegion { start, end })
        })
        .collect()
}

fn is_ignorable_section_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';')
}

// ---------------------------------------------------------------------------
// SQL detector
// ---------------------------------------------------------------------------

fn is_toml_header(line: &str) -> bool {
    let trimmed = line.split(['#', ';']).next().unwrap_or(line).trim();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return false;
    };
    let rest = rest.strip_prefix('[').unwrap_or(rest);
    let closing = if trimmed.starts_with("[[") { "]]" } else { "]" };
    let Some(header) = rest.strip_suffix(closing) else {
        return false;
    };
    let header = header
        .split_once('#')
        .map_or(header, |(name, _)| name)
        .trim();
    !header.is_empty() && !header.contains('[') && !header.contains(']')
}

// ---------------------------------------------------------------------------
// YAML indentation-based detector
// ---------------------------------------------------------------------------
