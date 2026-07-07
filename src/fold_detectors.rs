//! Pure fold-region detectors for brace-delimited and indentation-based
//! languages, intended for consumption by language-provider plugins.
//!
//! Two public functions share the `crate::fold::FoldRegion` output type:
//!
//! * `brace_fold` — character-level lexer-lite state machine that matches
//!   `{`/`}` pairs, skipping braces inside line/block comments, double-quoted
//!   strings (with `\"` escapes), Rust raw strings (`r"…"`, `r#"…"#`, …), and
//!   Go backtick strings.
//! * `indent_fold` — Python-style indentation detector.  A region spans from
//!   each compound-statement header (`def`/`class`/`if`/`for`/`while`/etc.) to
//!   the last more-indented line.  Continuation keywords (`else`/`elif`/
//!   `except`/`finally`) are not new headers.  Blank lines are transparent.
//!
//! Neither function knows about `App`, plugins, or IPC — they are pure
//! `&str` → `Vec<FoldRegion>` transformations, the same design as
//! `crate::yaml_fold::detect_fold_regions`.

use crate::fold::FoldRegion;

// ---------------------------------------------------------------------------
// Brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in brace-delimited languages (Rust, Go, C, Java,
/// JS, …).
///
/// Walks `text` character by character, maintaining a nesting stack for `{…}`
/// pairs.  Braces inside the following contexts are ignored:
///
/// * Line comments (`// …`)
/// * Block comments (`/* … */`)
/// * Double-quoted strings (`"…"` with `\"` escapes)
/// * Rust raw strings (`r"…"`, `r#"…"#`, `r##"…"##`, …)
/// * Go backtick strings (`` `…` ``)
///
/// Returns one region per `{…}` block that spans more than one line.  The
/// nesting stack is `Vec<usize>` (line number), so deeply nested files are
/// bounded only by available memory.
pub fn brace_fold(text: &str) -> Vec<FoldRegion> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return Vec::new();
    }

    #[derive(Clone, Copy)]
    enum St {
        Normal,
        LineCmt,
        BlockCmt,
        DqStr,
        RawStr(usize),
        BtStr,
    }

    let mut st = St::Normal;
    let mut line = 0usize;
    let mut stack: Vec<usize> = Vec::new();
    let mut regions: Vec<FoldRegion> = Vec::new();
    let mut i = 0;

    while i < len {
        let b = bytes[i];
        match st {
            St::Normal => match b {
                b'\n' => line += 1,
                b'{' => stack.push(line),
                b'}' => {
                    if let Some(start) = stack.pop() {
                        if line > start {
                            regions.push(FoldRegion { start, end: line });
                        }
                    }
                }
                b'/' if i + 1 < len => match bytes[i + 1] {
                    b'/' => {
                        st = St::LineCmt;
                        i += 1;
                    }
                    b'*' => {
                        st = St::BlockCmt;
                        i += 1;
                    }
                    _ => {}
                },
                b'"' => st = St::DqStr,
                b'`' => st = St::BtStr,
                b'r' => {
                    let prev_ident =
                        i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_');
                    if !prev_ident {
                        let mut j = i + 1;
                        let mut hashes = 0usize;
                        while j < len && bytes[j] == b'#' {
                            hashes += 1;
                            j += 1;
                        }
                        if j < len && bytes[j] == b'"' {
                            st = St::RawStr(hashes);
                            i = j;
                        }
                    }
                }
                _ => {}
            },
            St::LineCmt => {
                if b == b'\n' {
                    st = St::Normal;
                    line += 1;
                }
            }
            St::BlockCmt => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'*' && i + 1 < len && bytes[i + 1] == b'/' {
                    st = St::Normal;
                    i += 1;
                }
            }
            St::DqStr => {
                if b == b'\\' && i + 1 < len {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 1;
                } else if b == b'"' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
            St::RawStr(hashes) => {
                if b == b'\n' {
                    line += 1;
                } else if b == b'"' {
                    let mut j = i + 1;
                    let mut seen = 0usize;
                    while j < len && bytes[j] == b'#' && seen < hashes {
                        seen += 1;
                        j += 1;
                    }
                    if seen == hashes {
                        st = St::Normal;
                        i = j.wrapping_sub(1);
                    }
                }
            }
            St::BtStr => {
                if b == b'`' {
                    st = St::Normal;
                } else if b == b'\n' {
                    line += 1;
                }
            }
        }
        i += 1;
    }

    regions
}

// ---------------------------------------------------------------------------
// Indentation-based detector (Python)
// ---------------------------------------------------------------------------

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

#[cfg(test)]
#[path = "fold_detectors_test.rs"]
mod tests;
