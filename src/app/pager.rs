//! Applying piped-stdin content (pager mode) to the content pane.
//!
//! `crate::pager::parse_pager_bytes` does the pure parsing (line splitting,
//! binary/diff detection); [`App::open_pager_content`] wires the result into
//! the same content-pane fields `apply_file_load`/`apply_diff_load` populate,
//! so every existing content-pane feature (search, fold, word wrap, scroll)
//! works unmodified on piped input. There is no backing path: `current_file`
//! stays `None`, so there is nothing to watch or reload, and the tree pane is
//! collapsed to signal that navigation started from a pipe, not the tree.

use crate::pager::PagerContent;

use super::{diff_line_style, App, Focus};

impl App {
    /// Loads parsed piped-stdin content into the content pane: diff-shaped
    /// input goes through `parse_side_by_side` (so `GIT_PAGER=mantis` renders
    /// a navigable side-by-side diff out of the box); anything else is
    /// syntax-highlighted via `Highlighter::highlight_stdin`, honoring an
    /// explicit `--language` override before falling back to first-line
    /// sniffing. Collapses the tree pane and focuses the content pane, since
    /// there is no tree selection driving this view.
    pub fn open_pager_content(&mut self, parsed: PagerContent, language: Option<String>) {
        self.invalidate_pending_load();
        self.current_file = None;
        self.current_syntax = None;
        self.virtual_file = None;
        self.is_json = false;
        self.show_pretty_json = false;
        self.json_pretty_text = Vec::new();
        self.json_pretty_lines = Vec::new();
        self.viewing_revision = None;
        self.clear_fold_state();
        self.file_encoding = None;
        self.file_line_ending = None;
        self.set_content_scroll(0);
        self.content_hscroll = 0;
        self.active_line = 0;
        self.show_line_blame = false;
        self.clear_selection();
        self.in_file_search = None;

        self.is_diff = parsed.is_diff;
        if parsed.is_diff {
            self.diff_rows = crate::diff::parse_side_by_side(&parsed.content);
            self.highlighted = parsed
                .content
                .iter()
                .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
                .collect();
            self.content_title = Some(" <stdin> — diff ".to_string());
            // Pager mode's whole point is `GIT_PAGER=mantis` rendering a
            // side-by-side diff out of the box, regardless of the configured
            // default for diffs opened by navigating the tree.
            self.diff_side_by_side = true;
        } else {
            self.diff_rows = Vec::new();
            let (highlighted, syntax_name) = self
                .highlighter
                .highlight_stdin(language.as_deref(), &parsed.content);
            self.highlighted = highlighted;
            self.current_syntax = syntax_name;
            self.content_title = Some(" <stdin> ".to_string());
        }
        self.content = parsed.content;

        // No tree selection drives this view; collapse the tree pane (still
        // resizable via the splitter) and put focus on the content pane.
        self.tree_width = 0;
        self.focus = Focus::Content;
        self.set_file_watch(None);
    }
}

#[cfg(test)]
#[path = "pager_test.rs"]
mod tests;
