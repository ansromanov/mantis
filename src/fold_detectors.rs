//! Pure fold-region detectors for brace-delimited, indentation-based, and
//! YAML-style languages.
//!
//! Public functions share the `crate::fold::FoldRegion` output type:
//!
//! * `brace_fold` — character-level lexer-lite state machine that matches
//!   `{`/`}` pairs, skipping braces inside line/block comments, double-quoted
//!   strings (with `\"` escapes), Rust raw strings (`r"…"`, `r#"…"#`, …), and
//!   Go backtick strings.
//! * `brace_fold_with_brackets` — same state machine, additionally matching
//!   `[`/`]` pairs.  Used for JSON, where multiline arrays are as
//!   foldable as objects; kept separate from `brace_fold` so Rust/Go (where
//!   folding every multiline array literal would be noise) are unaffected.
//! * `shell_brace_fold` — shell-specific brace detector.  Matches `{`/`}`
//!   pairs, skipping braces inside `#` line comments, single-quoted strings
//!   (no escape processing), double-quoted strings (`\"` escapes), and
//!   heredocs (`<<WORD … WORD`).
//! * `hcl_brace_fold` — HCL-specific brace detector.  Matches `{`/`}`
//!   pairs, skipping braces inside `#`, `//`, and `/* … */` comments,
//!   double-quoted strings (`\"` escapes), and HCL heredocs
//!   (`<<EOF`, `<<'EOF'`, `<<"EOF"`, `<<-EOF`).
//! * `indent_fold` — Python-style indentation detector.  A region spans from
//!   each compound-statement header (`def`/`class`/`if`/`for`/`while`/etc.) to
//!   the last more-indented line.  Continuation keywords (`else`/`elif`/
//!   `except`/`finally`) are not new headers.  Blank lines are transparent.
//! * `yaml_fold` — YAML indentation detector. A region spans from any
//!   non-blank line to the last line more indented than it. This is the
//!   original built-in YAML fold algorithm; `crate::yaml_fold::detect_fold_regions`
//!   re-exports it so existing call sites are unaffected.
//! * `section_fold` — INI/TOML table-section detector. A region spans from a
//!   `[section]` header through its last meaningful line.
//! * `sql_fold` — SQL statement and block detector. It recognizes multiline
//!   semicolon-terminated statements and common procedural blocks.
//! * `template_fold` — delimiter-aware template block detector for Go templates,
//!   Helm, and Jinja2.
//! * `rust_symbols` — lightweight Rust declaration detector shared with the
//!   bundled Rust language provider.
//!
//! None of these functions know about `App` or any I/O — they are pure
//! transformations to `Vec<FoldRegion>`.

use crate::fold::FoldRegion;

// ---------------------------------------------------------------------------
// Brace-nesting detector
// ---------------------------------------------------------------------------

/// Detects foldable regions in YAML content by indentation nesting.
///
/// Delegates to `crate::yaml_fold::detect_fold_regions` — the original
/// built-in YAML fold algorithm — so there is a single YAML implementation.
pub fn yaml_fold(lines: &[impl AsRef<str>]) -> Vec<FoldRegion> {
    crate::yaml_fold::detect_fold_regions(lines)
}

mod brace;
mod dispatch;
mod indent;
mod sql;
mod symbols;
mod template;
pub use brace::{brace_fold, brace_fold_with_brackets, hcl_brace_fold, shell_brace_fold};
pub use dispatch::detect_for_path;
pub use indent::{indent_fold, section_fold};
pub use sql::sql_fold;
pub use symbols::rust_symbols;
pub use template::template_fold;

#[cfg(test)]
#[path = "fold_detectors_test.rs"]
mod tests;
