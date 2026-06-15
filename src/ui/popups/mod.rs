mod about;
mod blame;
mod command;
mod help;
mod history;
mod in_file;
mod search;
mod theme;
mod util;

pub(super) use about::draw_about;
pub(super) use blame::draw_blame_panel;
pub(super) use command::draw_command_palette;
pub(super) use help::draw_help;
pub(super) use history::draw_history;
pub(super) use in_file::draw_in_file_search;
pub(super) use search::draw_search;
pub(super) use theme::draw_theme;

#[cfg(test)]
#[path = "popups_test.rs"]
mod tests;
