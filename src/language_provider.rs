//! Language-provider abstraction for uniform highlight + fold capabilities.
//!
//! `LanguageProvider` is the single contract a language plugin must satisfy.
//! The host queries `capabilities()` once at init time and calls only the
//! methods the provider supports, so each language gets the right feature set
//! through one code path. The three LSP-era capabilities (`hover`,
//! `diagnostics`, `definition`) are declared here as always-`false` stubs so
//! adding a provider that implements them in 0.9 requires no protocol break.
//!
//! Two built-in providers ship with `tv`:
//! - [`SyntaxOnlyProvider`]: syntect highlighting, no fold.
//! - [`SyntaxFoldProvider`]: syntect highlighting plus indentation-based fold
//!   regions (the generalisation of the old YAML-only fold path).
//!
//! [`LanguageRegistry`] tracks which file extensions have fold support and
//! constructs the correct provider at file-open time (capability negotiation).

use std::path::Path;

/// Capabilities reported by a [`LanguageProvider`] at init time.
///
/// The host calls only the methods whose corresponding capability is `true`,
/// enabling graceful degradation: a provider that does not support fold simply
/// sets `fold: false` and the fold gutter is hidden.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    /// Provider can syntax-highlight file content.
    pub highlight: bool,
    /// Provider can compute indentation-based fold regions.
    pub fold: bool,
    /// Reserved for 0.9 LSP provider; always `false` in built-in providers.
    #[allow(dead_code)]
    pub hover: bool,
    /// Reserved for 0.9 LSP provider; always `false` in built-in providers.
    #[allow(dead_code)]
    pub diagnostics: bool,
    /// Reserved for 0.9 LSP provider; always `false` in built-in providers.
    #[allow(dead_code)]
    pub definition: bool,
}

/// Contract that every language plugin must satisfy.
///
/// The host calls `capabilities()` once at file-open time and then invokes
/// only the methods that are advertised as supported. Implementors must be
/// `Send + Sync` so they can be constructed on the worker thread.
pub trait LanguageProvider: Send + Sync {
    /// Returns the static set of capabilities this provider supports.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Syntax-highlights `lines` for the file at `path`.
    ///
    /// Returns one `Vec` of styled `(Style, String)` spans per input line.
    /// When `capabilities().highlight` is `false` the host does not call this
    /// method; returning an empty `Vec` is always safe.
    fn highlight(&self, path: &Path, lines: &[String])
        -> Vec<Vec<(ratatui::style::Style, String)>>;

    /// Computes indentation-based fold regions for `lines`.
    ///
    /// Called only when `capabilities().fold` is `true`. Returning an empty
    /// `Vec` disables the fold gutter for this file.
    fn fold_regions(&self, lines: &[String]) -> Vec<crate::yaml_fold::FoldRegion>;
}

/// A built-in provider that uses syntect for highlighting but does not support
/// fold regions. Used for all file types that have not opted in to fold.
pub struct SyntaxOnlyProvider {
    highlighter: crate::highlight::Highlighter,
}

impl SyntaxOnlyProvider {
    /// Creates a new `SyntaxOnlyProvider` wrapping the given `Highlighter`.
    pub fn new(highlighter: crate::highlight::Highlighter) -> Self {
        SyntaxOnlyProvider { highlighter }
    }
}

impl LanguageProvider for SyntaxOnlyProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            highlight: true,
            fold: false,
            hover: false,
            diagnostics: false,
            definition: false,
        }
    }

    fn highlight(
        &self,
        path: &Path,
        lines: &[String],
    ) -> Vec<Vec<(ratatui::style::Style, String)>> {
        self.highlighter.highlight(path, lines)
    }

    fn fold_regions(&self, _lines: &[String]) -> Vec<crate::yaml_fold::FoldRegion> {
        Vec::new()
    }
}

/// A built-in provider that uses syntect for highlighting and also computes
/// indentation-based fold regions (generalisation of the old YAML-only fold path).
pub struct SyntaxFoldProvider {
    highlighter: crate::highlight::Highlighter,
}

impl SyntaxFoldProvider {
    /// Creates a new `SyntaxFoldProvider` wrapping the given `Highlighter`.
    pub fn new(highlighter: crate::highlight::Highlighter) -> Self {
        SyntaxFoldProvider { highlighter }
    }
}

impl LanguageProvider for SyntaxFoldProvider {
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            highlight: true,
            fold: true,
            hover: false,
            diagnostics: false,
            definition: false,
        }
    }

    fn highlight(
        &self,
        path: &Path,
        lines: &[String],
    ) -> Vec<Vec<(ratatui::style::Style, String)>> {
        self.highlighter.highlight(path, lines)
    }

    fn fold_regions(&self, lines: &[String]) -> Vec<crate::yaml_fold::FoldRegion> {
        crate::yaml_fold::detect_fold_regions(lines)
    }
}

/// Tracks which file extensions have fold support and constructs the correct
/// [`LanguageProvider`] at file-open time (capability negotiation).
#[derive(Clone)]
pub struct LanguageRegistry {
    fold_extensions: std::collections::HashSet<String>,
}

impl LanguageRegistry {
    /// Creates a registry with YAML fold support enabled by default.
    pub fn new() -> Self {
        let mut fold_extensions = std::collections::HashSet::new();
        fold_extensions.insert("yaml".to_string());
        fold_extensions.insert("yml".to_string());
        Self { fold_extensions }
    }

    /// Registers additional file extensions for indentation-based fold support.
    ///
    /// Extensions are stored lowercase; the case of each argument is normalised
    /// before insertion so callers need not pre-lowercase them.
    pub fn register_fold_extensions(&mut self, exts: impl IntoIterator<Item = String>) {
        for ext in exts {
            self.fold_extensions.insert(ext.to_lowercase());
        }
    }

    /// Returns `true` when the file at `path` should have fold regions computed.
    pub fn wants_fold(&self, path: &Path) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        ext.map(|e| self.fold_extensions.contains(&e))
            .unwrap_or(false)
    }

    /// Returns the capabilities the registry would grant for `path`.
    pub fn capabilities_for(&self, path: &Path) -> ProviderCapabilities {
        let fold = self.wants_fold(path);
        ProviderCapabilities {
            highlight: true,
            fold,
            hover: false,
            diagnostics: false,
            definition: false,
        }
    }
}

impl Default for LanguageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "language_provider_test.rs"]
mod tests;
