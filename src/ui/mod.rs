//! ratatui rendering orchestration: the top-level `draw` entry point.
//!
//! `draw` is called once per frame and lays out the whole screen: it paints the
//! themed background, splits the area into the tree pane, content pane, and
//! status bar, and renders each by delegating to the `tree`, `content`, and
//! `statusbar` submodules. Modal overlays (help, search, history, theme picker,
//! command palette, about, blame) are drawn last, on top, following the same
//! precedence chain the input handlers use. Rendering is also where panel
//! geometry (`Rect`s and scroll offsets) is recorded back onto `App` for mouse
//! hit-testing, so this layer stays purely presentational.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::Block,
    Frame,
};

use crate::app::App;

mod content;
mod popups;
mod statusbar;
mod tree;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    // Paint the themed background; widgets that don't set their own bg inherit
    // it. With the default theme this is Color::Reset (the terminal default).
    f.render_widget(
        Block::default().style(Style::default().bg(app.theme.background)),
        area,
    );

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let tree_width = app.tree_width.clamp(5, 95);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tree_width),
            Constraint::Percentage(100 - tree_width),
        ])
        .split(vert[0]);

    // Record the 2-column splitter boundary for mouse hit-testing.
    app.splitter_area = Rect {
        x: horiz[0].right().saturating_sub(1),
        y: vert[0].y,
        width: 2,
        height: vert[0].height,
    };

    tree::draw_tree(f, app, horiz[0]);
    content::draw_content(f, app, horiz[1]);
    statusbar::draw_statusbar(f, app, vert[1]);

    if app.in_file_search.is_some() {
        popups::draw_in_file_search(f, app, horiz[1]);
    }

    if app.search.is_some() {
        popups::draw_search(f, app, area);
    }

    if app.command_palette.is_some() {
        popups::draw_command_palette(f, app, area);
    }

    if app.history.is_some() {
        popups::draw_history(f, app, area);
    }

    if app.theme_picker.is_some() {
        popups::draw_theme(f, app, area);
    }

    if app.recent_files.is_some() {
        popups::draw_recent(f, app, area);
    }

    if app.blame_panel {
        popups::draw_blame_panel(f, app, area);
    }

    if app.show_about {
        popups::draw_about(f, app, area);
    }

    if app.show_help {
        popups::draw_help(f, app, area);
    }
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
