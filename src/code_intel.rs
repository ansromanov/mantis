//! Plain data types for code intelligence shown in the content pane and
//! status bar: outline symbols and diagnostics.
//!
//! These used to live in the plugin protocol module. They are now ordinary
//! host types: symbols are produced by the built-in detectors in
//! [`crate::fold_detectors`], and the helpers here are shared by the status
//! bar breadcrumb, the symbol picker and the diagnostics gutter.
//!
//! Public items:
//! - [`Symbol`] and [`enclosing_symbol`]: outline entries and scope lookup.
//! - [`Diagnostic`] and [`DiagnosticSeverity`]: per-line issue records.

use serde::{Deserialize, Serialize};

/// Severity of one diagnostic.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum DiagnosticSeverity {
    /// A correctness issue that should be fixed.
    Error,
    /// A likely problem that may be context-dependent.
    Warning,
    /// Additional information about the source.
    Info,
    /// A low-priority suggestion.
    Hint,
}

impl DiagnosticSeverity {
    /// Lowercase label used in pickers and diagnostics messages.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
            Self::Hint => "hint",
        }
    }

    /// Single-cell marker used in the content gutter.
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Error => "x",
            Self::Warning => "!",
            Self::Info => "i",
            Self::Hint => ".",
        }
    }

    /// Relative importance used when several diagnostics share a line.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Error => 4,
            Self::Warning => 3,
            Self::Info => 2,
            Self::Hint => 1,
        }
    }
}

/// One read-only diagnostic for a source file.
///
/// Lines and columns are zero-based physical source coordinates. The UI
/// currently marks only `line`; optional end coordinates are retained for
/// consumers such as future range highlighting.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    /// Zero-based physical line containing the diagnostic.
    pub line: usize,
    /// Zero-based column within `line`.
    pub column: usize,
    /// Optional zero-based end line.
    #[serde(default)]
    pub end_line: Option<usize>,
    /// Optional zero-based end column.
    #[serde(default)]
    pub end_column: Option<usize>,
    /// Severity used for gutter markers and the status-bar badge.
    pub severity: DiagnosticSeverity,
    /// Human-readable diagnostic text.
    pub message: String,
    /// Short tool or rule identifier.
    pub source: String,
}

/// One named symbol discovered in a source file.
///
/// Records use zero-based physical source lines.
/// The host uses their ranges for outline navigation and cursor-scope display.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Symbol {
    /// The name shown in pickers and breadcrumbs.
    pub name: String,
    /// A short kind label such as `function`, `struct`, or `method`.
    pub kind: String,
    /// Zero-based physical source line where the symbol starts.
    pub line: usize,
    /// Zero-based physical source line where the symbol ends, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    /// Name of the containing symbol, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
}

/// Finds the narrowest symbol range containing `line`.
pub fn enclosing_symbol(symbols: &[Symbol], line: usize) -> Option<&Symbol> {
    symbols
        .iter()
        .filter(|symbol| symbol.line <= line && symbol.end_line.unwrap_or(symbol.line) >= line)
        .min_by_key(|symbol| {
            symbol
                .end_line
                .unwrap_or(symbol.line)
                .saturating_sub(symbol.line)
        })
}

#[cfg(test)]
#[path = "code_intel_test.rs"]
mod tests;
