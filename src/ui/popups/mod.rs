//! Modal overlay (popup) rendering, split into one submodule per popup.
//!
//! This module root collects every floating overlay the UI can draw on top of
//! the main layout: about, blame, command palette, help, history, recent files,
//! in-file search, file/content search, the theme picker, and the plugin
//! manager. Each lives in its own submodule and is re-exported as a `draw_*`
//! function for the UI orchestrator, which decides which (if any) is visible.
//! Shared layout helpers - notably `centered_rect` - live in `util`. Popups
//! generally `Clear` their region first, then render a bordered block, so they
//! visually float above the panes.

mod about;
mod blame;
mod command;
mod goto_line;
mod help;
mod history;
mod in_file;
mod line_blame;
mod plugin;
mod recent;
mod search;
mod theme;
mod tree_filter;
mod util;

pub(super) use about::draw_about;
pub(super) use blame::draw_blame_panel;
pub(super) use command::draw_command_palette;
pub(super) use goto_line::draw_goto_line;
pub(super) use help::draw_help;
pub(super) use history::draw_history;
pub(super) use in_file::draw_in_file_search;
pub(super) use line_blame::draw_line_blame;
pub(super) use plugin::draw_plugin_picker;
pub(super) use recent::draw_recent;
pub(super) use search::draw_search;
pub(super) use theme::draw_theme;
pub(super) use tree_filter::draw_tree_filter;

#[cfg(test)]
#[path = "popups_test.rs"]
mod tests;

#[cfg(test)]
#[path = "popups_plugin_test.rs"]
mod plugin_tests;
