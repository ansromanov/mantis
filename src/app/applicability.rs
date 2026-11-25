//! Whether an action applies to the current state (palette/menu greying and reasons).

use super::App;

impl App {
    /// Checks if a command is currently applicable based on the application state.
    /// Returns `Ok(())` if applicable, or `Err(reason)` if inapplicable.
    pub fn check_applicability(&self, action_id: &str) -> Result<(), &'static str> {
        let action = crate::actions::ACTIONS.iter().find(|a| a.id == action_id);
        let applicability = match action {
            Some(a) => a.applicability(),
            None => return Ok(()),
        };
        match applicability {
            crate::actions::Applicability::Always => Ok(()),
            crate::actions::Applicability::OpenFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                Ok(())
            }
            crate::actions::Applicability::JsonFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if !self.is_json {
                    return Err("requires JSON file");
                }
                if self.json_pretty_lines.is_empty() {
                    return Err("JSON file failed to parse");
                }
                Ok(())
            }
            crate::actions::Applicability::CsvFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if !self.is_csv {
                    return Err("requires CSV/TSV file");
                }
                if self.csv_table_lines.is_empty() {
                    return Err("CSV/TSV table failed to parse");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepo => {
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndFile => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndNoDiff => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                if self.is_diff {
                    return Err("not available in a diff");
                }
                if !self.has_text_cursor() {
                    return Err("not available (current file not rendered)");
                }
                Ok(())
            }
            crate::actions::Applicability::GitRepoAndDiffView => {
                if self.git_info.is_none() {
                    return Err("not in a git repo");
                }
                if !self.is_diff {
                    return Err("requires diff view");
                }
                Ok(())
            }
            crate::actions::Applicability::DiffView => {
                if !self.is_diff {
                    return Err("requires diff view");
                }
                Ok(())
            }
            crate::actions::Applicability::FoldRegions => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self.fold_regions.is_empty() && !self.is_jsonl {
                    return Err("no fold regions in file");
                }
                Ok(())
            }
            crate::actions::Applicability::RenderedMarkdownActive => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if !self.rendered_active {
                    return Err("not available (current file not rendered)");
                }
                Ok(())
            }
            crate::actions::Applicability::Symbols => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self
                    .current_file
                    .as_deref()
                    .and_then(|path| self.symbols.get(path))
                    .is_none_or(Vec::is_empty)
                {
                    return Err("no symbols in file");
                }
                Ok(())
            }
            crate::actions::Applicability::Diagnostics => {
                if self.current_file.is_none() {
                    return Err("no file is open");
                }
                if self
                    .current_file
                    .as_deref()
                    .and_then(|path| self.file_diagnostics.get(path))
                    .is_none_or(Vec::is_empty)
                {
                    return Err("no diagnostics in file");
                }
                Ok(())
            }
            crate::actions::Applicability::GitMode => {
                if !self.git_mode {
                    return Err("requires git mode");
                }
                Ok(())
            }
        }
    }
}
