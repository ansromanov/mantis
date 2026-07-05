//! Pager-mode stdin ingestion: piped-input detection and content sniffing.
//!
//! When mantis is invoked with no path argument and stdin is not a terminal
//! (`git diff | mantis`, `kubectl logs pod | mantis`), it reads the pipe to
//! EOF instead of walking a directory. [`is_piped_stdin`] makes that decision,
//! [`read_stdin_bytes`] performs the (blocking, read-to-EOF) read, and
//! [`parse_pager_bytes`] turns the raw bytes into a [`PagerContent`]: line
//! splitting, binary detection, and diff sniffing (`diff --git` / `@@` / a
//! `---`+`+++` pair), so the caller can decide between the diff renderer and
//! plain syntax-highlighted text. This module owns only the pure/IO parsing;
//! `App::open_pager_content` (in `app::pager`) applies the result to the
//! content pane, and syntax detection for non-diff content lives in
//! `Highlighter::highlight_stdin`.

use std::io::{self, Read};

use crossterm::tty::IsTty;

use crate::file::{build_binary_placeholder_content, is_binary_bytes};

/// How many leading lines are scanned for diff markers. Bounded so a huge
/// piped file doesn't pay an `O(n)` scan just to rule out being a diff.
const DIFF_SNIFF_LINES: usize = 50;

/// Returns `true` when stdin is not connected to a terminal, i.e. piped or
/// redirected from a file. Used to decide whether to enter pager mode.
pub fn is_piped_stdin() -> bool {
    !io::stdin().is_tty()
}

/// Reads stdin to EOF. Blocking: a first version reads the whole (bounded)
/// input rather than streaming, per the pager-mode design (see issue #489).
pub fn read_stdin_bytes() -> io::Result<Vec<u8>> {
    let mut buf = Vec::new();
    io::stdin().lock().read_to_end(&mut buf)?;
    Ok(buf)
}

/// Parsed piped-stdin content, ready for `App::open_pager_content`.
pub struct PagerContent {
    /// The input split into lines, normalized to LF. A binary or empty input
    /// is represented as a single placeholder line, mirroring `compute_file_load`.
    pub content: Vec<String>,
    /// Whether `content` looks like unified-diff text and should render
    /// through `crate::diff::parse_side_by_side` instead of plain highlighting.
    pub is_diff: bool,
}

/// Splits raw stdin bytes into lines and classifies them as diff or plain
/// text. Binary input (a NUL byte in the scanned prefix) short-circuits to a
/// placeholder line, same convention as `compute_file_load`.
pub fn parse_pager_bytes(bytes: &[u8]) -> PagerContent {
    if is_binary_bytes(bytes) {
        return PagerContent {
            content: build_binary_placeholder_content(None, bytes),
            is_diff: false,
        };
    }
    let text = String::from_utf8_lossy(bytes);
    let text = if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.into_owned()
    };
    let mut content: Vec<String> = text.lines().map(|l| l.to_owned()).collect();
    if content.is_empty() {
        content = vec!["[empty file]".into()];
    }
    let is_diff = looks_like_diff(&content);
    PagerContent { content, is_diff }
}

/// Heuristic diff sniff: a `diff --git`/`diff --cc` header, a hunk header
/// (`@@ -`), or a `--- `/`+++ ` file-marker pair (plain `diff -u` output,
/// which has no `diff --git` line) within the first `DIFF_SNIFF_LINES`.
fn looks_like_diff(lines: &[String]) -> bool {
    let scanned = &lines[..lines.len().min(DIFF_SNIFF_LINES)];
    let header_hit = scanned.iter().any(|l| {
        l.starts_with("diff --git ") || l.starts_with("diff --cc ") || l.starts_with("@@ -")
    });
    if header_hit {
        return true;
    }
    scanned
        .windows(2)
        .any(|w| w[0].starts_with("--- ") && w[1].starts_with("+++ "))
}

#[cfg(test)]
#[path = "pager_test.rs"]
mod tests;
