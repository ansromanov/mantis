//! Chooses the built-in fold detector for a file from its extension or shebang.
//!
//! Folding runs in the file loader, so fold markers are present on the first
//! painted frame.
//!
//! YAML is intentionally absent: the loader already runs
//! [`crate::yaml_fold`] for it and also needs anchor/alias counts.
//! JSON callers must pass the pretty-printed lines (the text actually
//! displayed), because fold line numbers are matched against the display.

use std::path::Path;

use super::{
    brace_fold, brace_fold_with_brackets, hcl_brace_fold, indent_fold, section_fold,
    shell_brace_fold, sql_fold, template_fold,
};
use crate::fold::FoldRegion;

/// Detector family for a file, decided by [`detector_for`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detector {
    Brace,
    BraceBrackets,
    Section,
    Shell,
    Hcl,
    Sql,
    Template,
    Indent,
}

/// Picks the detector for `path`, falling back to the shebang interpreter for
/// extension-less scripts. Returns `None` for files with no built-in folding.
pub fn detector_for(path: &Path, shebang: Option<&str>) -> Option<Detector> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let detector = match ext.as_deref() {
        Some("rs" | "go" | "css" | "scss" | "less") => Detector::Brace,
        Some("ts" | "tsx" | "js" | "jsx" | "mts" | "cts" | "mjs" | "cjs") => Detector::Brace,
        Some("json") => Detector::BraceBrackets,
        Some("toml" | "ini" | "service" | "timer" | "conf" | "properties" | "cfg" | "desktop") => {
            Detector::Section
        }
        Some("sh" | "bash" | "zsh") => Detector::Shell,
        Some("tf" | "tfvars" | "hcl") => Detector::Hcl,
        Some("sql") => Detector::Sql,
        Some("tpl" | "tmpl" | "template" | "j2" | "jinja" | "jinja2") => Detector::Template,
        Some("py" | "pyi") => Detector::Indent,
        _ => match shebang {
            Some("bash" | "sh" | "zsh") => Detector::Shell,
            _ => return None,
        },
    };
    Some(detector)
}

/// Runs the detector chosen for `path` over `text`. `None` when the language
/// has no built-in detector.
pub fn detect_for_path(path: &Path, shebang: Option<&str>, text: &str) -> Option<Vec<FoldRegion>> {
    let detector = detector_for(path, shebang)?;
    Some(match detector {
        Detector::Brace => brace_fold(text),
        Detector::BraceBrackets => brace_fold_with_brackets(text),
        Detector::Section => section_fold(text),
        Detector::Shell => shell_brace_fold(text),
        Detector::Hcl => hcl_brace_fold(text),
        Detector::Sql => sql_fold(text),
        Detector::Template => template_fold(text),
        Detector::Indent => indent_fold(text),
    })
}

#[cfg(test)]
#[path = "dispatch_test.rs"]
mod tests;
