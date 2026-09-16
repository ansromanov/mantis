//! ratatui rendering orchestration: the top-level `draw` entry point.
//!
//! `draw` is called once per frame and lays out the whole screen: it paints the
//! themed background, splits the area into the optional menu row, left column
//! (the workspace list when several tabs are open, then the tree), content
//! pane, and status bar, and renders each by delegating to `tree`,
//! `content`, and `statusbar` submodules. Modal overlays (help, search, history, theme picker,
//! command palette, about, blame) are drawn last, on top, following the same
//! precedence chain the input handlers use. Rendering is also where panel
//! geometry (`Rect`s and scroll offsets) is recorded back onto `App` for mouse
//! hit-testing, so this layer stays purely presentational.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Paragraph},
    Frame,
};

use crate::app::App;

mod content;
pub(crate) mod menu_bar;
pub(crate) mod popups;
pub(crate) mod statusbar;
pub(crate) mod tabstrip;
pub mod tree;

const MIN_LAYOUT_WIDTH: u16 = 80;
const MIN_LAYOUT_HEIGHT: u16 = 6;

/// Draws the whole workspace: the regular single-root frame for the active
/// tab, with a ` Workspaces ` list stacked above its file tree when more than
/// one tab is open or the "open project as new tab" prompt is active. With
/// exactly one tab and no prompt open, this renders identically to calling
/// [`draw`] directly.
pub fn draw_workspace(f: &mut Frame, tabs: &mut crate::workspace::Tabs) {
    if tabs.apps.len() > 1 || tabs.new_tab_prompt.is_some() {
        let list = tabstrip::WorkspaceList::from_tabs(tabs);
        let area = f.area();
        tabs.strip_area = draw_area(f, tabs.active_app_mut(), area, Some(&list));
        tabs.ensure_active_visible();
    } else {
        tabs.strip_area = Rect::default();
        draw(f, tabs.active_app_mut());
    }
    if tabs.tab_picker.is_some() {
        popups::draw_tab_picker(f, tabs);
    }
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();
    draw_area(f, app, area, None);
}

/// Draws one tab's frame into `area`. When `workspaces` is given, the list is
/// drawn at the top of the left column and its `Rect` is returned (otherwise
/// `Rect::default()`), so overlays drawn afterwards still sit on top of it.
fn draw_area(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    workspaces: Option<&tabstrip::WorkspaceList>,
) -> Rect {
    // Paint the themed background; widgets that don't set their own bg inherit
    // it. With the default theme this is Color::Reset (the terminal default).
    f.render_widget(
        Block::default().style(Style::default().bg(app.theme.background)),
        area,
    );

    let menu_visible = app.config.ui.menu_bar || app.menu_bar_state.is_some();
    let min_height = MIN_LAYOUT_HEIGHT + u16::from(menu_visible);
    // Allocate the status bar its configured number of rows (1 or 2), so a
    // two-row bar has room to spread segments instead of eliding them.
    let sb_height = app.config.statusbar.height.clamp(1, 2) as u16;
    let vert = if menu_visible {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(sb_height),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(sb_height)])
            .split(area)
    };
    let menu_area = if menu_visible {
        vert[0]
    } else {
        app.menu_bar_area = Rect::default();
        app.menu_dropdown_area = Rect::default();
        Rect::default()
    };
    let body_area = if menu_visible { vert[1] } else { vert[0] };
    let status_area = if menu_visible { vert[2] } else { vert[1] };

    if area.width < MIN_LAYOUT_WIDTH || area.height < min_height {
        app.tree_area = Rect::default();
        app.content_area = Rect::default();
        app.splitter_area = Rect::default();
        let resize_message = match (area.width < MIN_LAYOUT_WIDTH, area.height < min_height) {
            (true, true) => format!(
                "Terminal too small: {MIN_LAYOUT_WIDTH} columns x {min_height} rows minimum."
            ),
            (true, false) => {
                format!("Terminal too narrow. Resize to at least {MIN_LAYOUT_WIDTH} columns.")
            }
            (false, true) => {
                format!("Terminal too short. Resize to at least {min_height} rows.")
            }
            (false, false) => unreachable!(),
        };
        f.render_widget(
            Paragraph::new(resize_message)
                .alignment(Alignment::Center)
                .style(Style::default().fg(app.theme.text)),
            body_area,
        );
        app.statusbar_area = status_area;
        app.statusbar_segments = statusbar::draw_statusbar(f, app, status_area);
        if menu_visible {
            menu_bar::draw_menu_bar(f, app, menu_area);
        }
        return Rect::default();
    }

    let tree_width = app.tree_width.clamp(5, 95);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(tree_width),
            Constraint::Percentage(100 - tree_width),
        ])
        .split(body_area);

    // Record the 2-column splitter boundary for mouse hit-testing.
    app.splitter_area = Rect {
        x: horiz[0].right().saturating_sub(1),
        y: body_area.y,
        width: 2,
        height: body_area.height,
    };

    let mut workspace_area = Rect::default();
    let tree_area = match workspaces {
        Some(list) => {
            let column = horiz[0];
            let height = list.height_for(column.height).min(column.height);
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(height), Constraint::Min(0)])
                .split(column);
            workspace_area = tabstrip::draw_workspace_list(f, &app.theme, list, split[0]);
            split[1]
        }
        None => horiz[0],
    };

    tree::draw_tree(f, app, tree_area);
    content::draw_content(f, app, horiz[1]);
    app.statusbar_area = status_area;
    app.statusbar_segments = statusbar::draw_statusbar(f, app, status_area);

    if app.in_file_search_open || app.json_query.is_some() {
        popups::draw_in_file_search(f, app, horiz[1]);
    }

    if app.filter_bar.is_some() {
        popups::draw_filter_bar(f, app, horiz[1]);
    }

    if app.tree_filter.is_some() {
        popups::draw_tree_filter(f, app, tree_area);
    }

    if app.revision_picker.is_some() {
        popups::draw_revision_picker(f, app, area);
    }

    if app.worktree_picker.is_some() {
        popups::draw_worktree_picker(f, app, area);
    }

    if app.goto_line.is_some() {
        popups::draw_goto_line(f, app, horiz[1]);
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

    if app.repo_log.is_some() {
        popups::draw_repo_log(f, app, area);
    }

    if app.plugin_picker.is_some() {
        popups::draw_plugin_picker(f, app, area);
    }

    if app.theme_picker.is_some() {
        popups::draw_theme(f, app, area);
    }

    if app.recent_files.is_some() {
        popups::draw_recent(f, app, area);
    }

    if app.bookmarks.is_some() {
        popups::draw_bookmarks(f, app, area);
    }

    if app.bug_report.is_some() {
        popups::draw_bug_report(f, app, area);
    }

    if app.show_telemetry_notice {
        popups::draw_telemetry_notice(f, app, area);
    }

    if app.show_about {
        popups::draw_about(f, app, area);
    }

    if app.show_help {
        popups::draw_help(f, app, area);
    }

    if app.context_menu.is_some() {
        popups::draw_context_menu(f, app, area);
    }

    if app.show_welcome {
        popups::draw_welcome(f, app, area);
    }

    if menu_visible {
        menu_bar::draw_menu_bar(f, app, menu_area);
    }
    workspace_area
}

#[cfg(test)]
#[path = "mod_test.rs"]
mod tests;
