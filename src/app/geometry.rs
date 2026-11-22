//! Screen geometry recorded by the renderer for mouse hit-testing.
//!
//! Drawing writes the `Rect` of each panel/popup (and the scroll offset of each
//! list popup) back here every frame; mouse handlers read them to decide what a
//! click or wheel event landed on. Grouping them keeps `App` itself focused on
//! behavioural state and makes it obvious which fields are renderer output.

use ratatui::layout::Rect;

#[derive(Default)]
pub struct Geometry {
    /// Hit area of the recent-files list recorded during the last render.
    pub recent_area: Rect,
    /// Scroll offset of the recent-files list recorded during the last render.
    pub recent_offset: usize,
    /// Hit area of the context menu recorded during the last render, used to
    /// hit-test item clicks and click-away dismissal.
    pub context_menu_area: Rect,
    /// Hit area of the optional menu row.
    pub menu_bar_area: Rect,
    /// Hit area of the currently open menu dropdown.
    pub menu_dropdown_area: Rect,
    /// Status-bar hit area recorded during the last render.
    pub statusbar_area: Rect,
    pub help_area: Rect,
    pub bug_report_area: Rect,
    pub bug_report_preview_area: Rect,
    pub worktree_picker_area: Rect,
    pub worktree_picker_offset: usize,
    pub welcome_area: Rect,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
    pub content_area: Rect,
    /// Cell rectangle the content pane reserved for the inline image during the
    /// last render. `Rect::default()` when no image is being shown.
    pub image_area: Rect,
    /// Hit area of the blame annotation strip recorded during the last render.
    /// `Rect::default()` when blame is not shown or there is no blame data.
    pub blame_area: Rect,
    /// Hit area of the diff info and actions header bar at the top of the diff pane.
    pub diff_header_area: Rect,
    pub search_area: Rect,
    pub search_offset: usize,
    pub command_palette_area: Rect,
    pub command_palette_offset: usize,
    pub history_area: Rect,
    pub history_offset: usize,
    /// Hit area of the repo log list recorded during the last render.
    pub repo_log_area: Rect,
    /// Scroll offset of the repo log list recorded during the last render.
    pub repo_log_offset: usize,
    pub theme_area: Rect,
    pub theme_offset: usize,
    /// Hit area of the revision picker list recorded during the last render.
    pub revision_picker_area: Rect,
    /// Scroll offset of the revision picker list recorded during the last render.
    pub revision_picker_offset: usize,
    /// Hit area of the splitter bar between tree and content panes.
    pub splitter_area: Rect,
    /// Breadcrumb segment areas recorded during the last render, used for mouse
    /// hit-testing. Each entry is (target_directory_path, clickable_rect).
    pub breadcrumb_areas: Vec<(std::path::PathBuf, Rect)>,
}
