use ratatui::{
    layout::{Constraint, Direction, Layout},
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

    if app.show_about {
        popups::draw_about(f, app, area);
    }

    if app.show_help {
        popups::draw_help(f, app, area);
    }
}
