//! CSV and TSV parsing and table formatting.
//!
//! Delimited data (`.csv` and `.tsv` files) is difficult to read as raw text
//! because columns do not align vertically. This module provides pure functions
//! to detect CSV/TSV paths, parse RFC 4180 compliant delimited records (including
//! quoted fields, escaped quotes, embedded delimiters, and multiline cells),
//! and format the parsed records as aligned, bordered tables for terminal viewing.
//!
//! Public items owned by this module:
//! - [`is_csv_file`]: returns whether a file path has a `.csv` or `.tsv` extension.
//! - [`delimiter_for_path`]: determines the delimiter character (`,` or `\t`) for a path.
//! - [`parse_delimited`]: parses raw CSV or TSV text into normalized rows and columns.
//! - [`format_table_lines`]: formats rows into aligned table lines with box borders.

use std::path::Path;
use unicode_width::UnicodeWidthStr;

/// Returns whether `path` has a CSV or TSV file extension (case-insensitive).
pub fn is_csv_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("csv") || ext.eq_ignore_ascii_case("tsv"))
}

/// Returns the field delimiter character for `path` (`\t` for TSV, `,` for CSV and others).
pub fn delimiter_for_path(path: &Path) -> char {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            if ext.eq_ignore_ascii_case("tsv") {
                '\t'
            } else {
                ','
            }
        })
        .unwrap_or(',')
}

/// Parses delimited content (CSV or TSV) according to RFC 4180 rules.
///
/// Handles quoted fields (`"..."`), escaped double quotes (`""`), embedded
/// delimiters, and embedded newlines (flattened to `↵` so table rows remain
/// single-line). Uneven row lengths are padded with empty strings to the maximum
/// column count. Trailing blank lines are ignored.
pub fn parse_delimited(content: &str, delimiter: char) -> Vec<Vec<String>> {
    if content.is_empty() {
        return Vec::new();
    }

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut current_row: Vec<String> = Vec::new();
    let mut current_field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped double quote
                    chars.next();
                    current_field.push('"');
                } else {
                    in_quotes = false;
                }
            } else if ch == '\r' {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                current_field.push('↵');
            } else if ch == '\n' {
                current_field.push('↵');
            } else {
                current_field.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == delimiter {
            current_row.push(std::mem::take(&mut current_field));
        } else if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            current_row.push(std::mem::take(&mut current_field));
            rows.push(std::mem::take(&mut current_row));
        } else if ch == '\n' {
            current_row.push(std::mem::take(&mut current_field));
            rows.push(std::mem::take(&mut current_row));
        } else {
            current_field.push(ch);
        }
    }

    // Flush any pending field / row
    if !current_field.is_empty() || !current_row.is_empty() {
        current_row.push(std::mem::take(&mut current_field));
        rows.push(std::mem::take(&mut current_row));
    }

    // Strip trailing empty rows caused by a final newline
    while let Some(last) = rows.last() {
        if last.len() == 1 && last[0].is_empty() && rows.len() > 1 {
            rows.pop();
        } else {
            break;
        }
    }

    if rows.is_empty() {
        return Vec::new();
    }

    // Pad ragged rows to maximum column count
    let max_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if max_cols == 0 {
        return Vec::new();
    }
    for row in &mut rows {
        while row.len() < max_cols {
            row.push(String::new());
        }
    }

    rows
}

/// Formats parsed CSV/TSV rows into aligned table lines with box-drawing borders.
///
/// If `rows` is empty, returns a single placeholder `"[empty table]"`.
/// The first row is formatted as a header row followed by a separator line.
pub fn format_table_lines(rows: &[Vec<String>]) -> Vec<String> {
    if rows.is_empty() {
        return vec!["[empty table]".into()];
    }

    let col_count = rows[0].len();
    if col_count == 0 {
        return vec!["[empty table]".into()];
    }

    let mut col_widths = vec![1; col_count];
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < col_count {
                col_widths[i] = col_widths[i].max(UnicodeWidthStr::width(cell.as_str()));
            }
        }
    }

    let mut lines = Vec::new();

    // Top border
    lines.push(table_border('┌', '─', '┬', '┐', &col_widths));

    // Header row (row 0)
    let mut header_line = String::from("│");
    for (i, w) in col_widths.iter().enumerate() {
        let text = rows[0].get(i).map(|s| s.as_str()).unwrap_or("");
        let pad = w.saturating_sub(UnicodeWidthStr::width(text));
        header_line.push_str(&format!(" {text}{} │", " ".repeat(pad)));
    }
    lines.push(header_line);

    // Header separator (if >1 row)
    if rows.len() > 1 {
        lines.push(table_border('├', '─', '┼', '┤', &col_widths));

        // Data rows (rows 1..)
        for row in &rows[1..] {
            let mut row_line = String::from("│");
            for (i, w) in col_widths.iter().enumerate() {
                let text = row.get(i).map(|s| s.as_str()).unwrap_or("");
                let pad = w.saturating_sub(UnicodeWidthStr::width(text));
                row_line.push_str(&format!(" {text}{} │", " ".repeat(pad)));
            }
            lines.push(row_line);
        }
    }

    // Bottom border
    lines.push(table_border('└', '─', '┴', '┘', &col_widths));

    lines
}

fn table_border(left: char, fill: char, mid: char, right: char, widths: &[usize]) -> String {
    let mut s = String::from(left);
    for (i, w) in widths.iter().enumerate() {
        for _ in 0..(*w + 2) {
            s.push(fill);
        }
        s.push(if i < widths.len() - 1 { mid } else { right });
    }
    s
}

#[cfg(test)]
#[path = "csv_table_test.rs"]
mod tests;
