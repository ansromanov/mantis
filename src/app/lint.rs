//! Background Python diagnostics for the open file.
//!
//! When a Python file finishes loading, [`App::request_file_diagnostics`] starts
//! a short-lived thread that runs `ruff` (see [`crate::lint`]) and sends the
//! result back over a channel. [`App::drain_diagnostics`], called from `tick`,
//! installs it on the UI thread. Each request carries a sequence number so a
//! slow reply for an older load never overwrites the diagnostics of a newer one,
//! and the file view is never blocked waiting on the linter.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::App;
use crate::code_intel::{Diagnostic, DiagnosticSeverity};

/// One finished lint run: `(request id, file, diagnostics)`.
pub(crate) type LintResult = (u64, PathBuf, Vec<Diagnostic>);

impl App {
    /// Clears the file's old diagnostics and, for Python files, starts a
    /// background lint run whose result `drain_diagnostics` will apply.
    pub(crate) fn request_file_diagnostics(&mut self, path: &Path) {
        self.clear_diagnostics_for_path(path);
        if !crate::lint::is_lintable(path) {
            return;
        }
        self.diagnostic_seq = self.diagnostic_seq.wrapping_add(1);
        let id = self.diagnostic_seq;
        self.latest_diagnostic_requests
            .insert(path.to_path_buf(), id);
        let tx = self.diagnostics_tx.clone();
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            let diagnostics = crate::lint::run_ruff(&path);
            // The receiver is gone when the app has quit; nothing to do then.
            let _ = tx.send((id, path, diagnostics));
        });
    }

    /// Applies completed lint runs. Stale replies (superseded by a newer
    /// request for the same file) are ignored.
    pub(crate) fn drain_diagnostics(&mut self) {
        while let Ok((id, path, diagnostics)) = self.diagnostics_rx.try_recv() {
            if self.latest_diagnostic_requests.get(&path) != Some(&id) {
                continue;
            }
            self.latest_diagnostic_requests.remove(&path);
            self.apply_diagnostics(path, diagnostics);
        }
    }

    fn apply_diagnostics(&mut self, path: PathBuf, diagnostics: Vec<Diagnostic>) {
        if diagnostics.is_empty() {
            return;
        }
        let mut line_severity: HashMap<usize, DiagnosticSeverity> = HashMap::new();
        for diagnostic in &diagnostics {
            line_severity
                .entry(diagnostic.line)
                .and_modify(|severity| {
                    if diagnostic.severity.rank() > severity.rank() {
                        *severity = diagnostic.severity;
                    }
                })
                .or_insert(diagnostic.severity);
        }
        self.file_diagnostics
            .insert(path.clone(), diagnostics.clone());
        self.file_diagnostic_lines
            .insert(path.clone(), line_severity);
        if self.current_file.as_deref() == Some(path.as_path()) {
            if let Some(picker) = self
                .command_palette
                .as_mut()
                .filter(|palette| {
                    palette.route == crate::command_palette::PaletteRoute::Diagnostics
                })
                .and_then(|palette| palette.route_diagnostics.as_mut())
            {
                picker.set_diagnostics(diagnostics);
            }
        }
    }
}

#[cfg(test)]
#[path = "lint_test.rs"]
mod tests;
