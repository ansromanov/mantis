//! The stack of overlays layered above the tree and content panes.
//!
//! Every popup, picker, bar and menu used to be a separate `Option`/`bool` field
//! on `App`, and three places (key dispatch, mouse dispatch, drawing) each
//! hard-coded their own `if … else if …` chain over them, in three slightly
//! different orders. An overlay drawn on top could therefore be starved of input
//! by one drawn below it. This module is the single source of truth for which
//! overlays exist and how they stack: [`Overlay::STACK`] lists them from the
//! bottom of the z-order to the top, [`App::open_overlays`] returns the ones that
//! are currently open in that order, and [`App::top_overlay`] is the one that
//! receives input.
//!
//! Public items:
//! - [`Overlay`]: one kind of overlay.
//! - [`App::is_overlay_open`], [`App::open_overlays`], [`App::top_overlay`].

use super::App;

/// One kind of overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Overlay {
    /// In-file search bar or the JSON query bar (they share one slot).
    InFileBar,
    FilterBar,
    TreeFilter,
    RevisionPicker,
    WorktreePicker,
    GotoLine,
    Search,
    CommandPalette,
    History,
    RepoLog,
    ThemePicker,
    RecentFiles,
    Bookmarks,
    BugReport,
    About,
    Help,
    ContextMenu,
    Welcome,
    /// The open menu-bar dropdown. It is painted by the menu bar itself (which is
    /// always visible when enabled), but it receives input above everything else.
    MenuBar,
}

impl Overlay {
    /// Every overlay, from the bottom of the z-order to the top. Later entries are
    /// drawn over earlier ones and take input first.
    pub(crate) const STACK: &'static [Overlay] = &[
        Overlay::InFileBar,
        Overlay::FilterBar,
        Overlay::TreeFilter,
        Overlay::RevisionPicker,
        Overlay::WorktreePicker,
        Overlay::GotoLine,
        Overlay::Search,
        Overlay::CommandPalette,
        Overlay::History,
        Overlay::RepoLog,
        Overlay::RecentFiles,
        Overlay::Bookmarks,
        Overlay::BugReport,
        Overlay::About,
        Overlay::Help,
        Overlay::ContextMenu,
        // Above the context menu on purpose: a context menu left open by a stale
        // state must never swallow input meant for the theme picker.
        Overlay::ThemePicker,
        Overlay::Welcome,
        Overlay::MenuBar,
    ];
}

impl App {
    /// Whether `overlay` is currently open.
    pub(crate) fn is_overlay_open(&self, overlay: Overlay) -> bool {
        match overlay {
            Overlay::InFileBar => self.in_file_search_open || self.json_query.is_some(),
            Overlay::FilterBar => self.filter_bar.is_some(),
            Overlay::TreeFilter => self.tree_filter.is_some(),
            Overlay::RevisionPicker => self.revision_picker.is_some(),
            Overlay::WorktreePicker => self.worktree_picker.is_some(),
            Overlay::GotoLine => self.goto_line.is_some(),
            Overlay::Search => self.search.is_some(),
            Overlay::CommandPalette => self.command_palette.is_some(),
            Overlay::History => self.history.is_some(),
            Overlay::RepoLog => self.repo_log.is_some(),
            Overlay::ThemePicker => self.theme_picker.is_some(),
            Overlay::RecentFiles => self.recent_files.is_some(),
            Overlay::Bookmarks => self.bookmarks.is_some(),
            Overlay::BugReport => self.bug_report.is_some(),
            Overlay::About => self.show_about,
            Overlay::Help => self.show_help,
            Overlay::ContextMenu => self.context_menu.is_some(),
            Overlay::Welcome => self.show_welcome,
            Overlay::MenuBar => self.menu_bar_state.is_some(),
        }
    }

    /// The open overlays, bottom of the z-order first.
    pub(crate) fn open_overlays(&self) -> Vec<Overlay> {
        Overlay::STACK
            .iter()
            .copied()
            .filter(|&overlay| self.is_overlay_open(overlay))
            .collect()
    }

    /// The topmost open overlay: the one that receives input. `None` when only
    /// the tree and content panes are showing.
    pub(crate) fn top_overlay(&self) -> Option<Overlay> {
        Overlay::STACK
            .iter()
            .rev()
            .copied()
            .find(|&overlay| self.is_overlay_open(overlay))
    }
}

#[cfg(test)]
#[path = "overlay_test.rs"]
mod tests;
