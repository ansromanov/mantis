//! Fuzzy-picker overlay types, one submodule per picker.
//!
//! `ThemePicker`, `RecentFilesState`, `GotoLineState`, `SymbolPicker`,
//! `DiagnosticPicker`, `TabPicker`, `WorktreePicker`, `TreeFilter`,
//! `RevisionPicker`, `InFileSearch`, `BugReportState` and `FilterBarState`, each
//! with its `ListPicker` implementation. Everything is re-exported here so the
//! public paths stay `crate::search::pickers::*` / `crate::search::*`.

mod bug_report;
mod diagnostic;
mod filter_bar;
mod goto_line;
mod in_file;
mod recent;
mod revision;
mod symbol;
mod tab;
mod theme;
mod tree_filter;
mod worktree;

pub use bug_report::{BugReportFocus, BugReportState};
pub use diagnostic::DiagnosticPicker;
pub use filter_bar::FilterBarState;
pub use goto_line::GotoLineState;
pub use in_file::{InFileMatch, InFileSearch};
pub use recent::RecentFilesState;
pub use revision::{RevisionItem, RevisionPicker, RevisionTab};
pub use symbol::SymbolPicker;
pub use tab::{TabPicker, TabPickerItem};
pub use theme::ThemePicker;
pub use tree_filter::TreeFilter;
pub use worktree::WorktreePicker;

#[cfg(test)]
#[path = "../pickers_test.rs"]
mod tests;
