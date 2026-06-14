use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::{App, Focus};
use crate::git::GitStatus;

/// Renders the file tree panel. Iterates `app.nodes`, drawing indentation,
/// expand/collapse arrows, and git-status coloring. Records `tree_area` and
/// `tree_offset` for mouse hit-testing.
pub(super) fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let focused = matches!(app.focus, Focus::Tree)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(theme.accent)
    } else {
        Style::default().fg(theme.dim)
    };

    let git_suffix = if app.git_mode {
        if app.git_mode_flat {
            " [git:flat]"
        } else {
            " [git]"
        }
    } else {
        ""
    };
    let title = format!(
        " {}{} ",
        app.root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string()),
        git_suffix
    );

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let items: Vec<ListItem> = app
        .nodes
        .iter()
        .map(|node| {
            let indent = "  ".repeat(node.depth);
            let arrow = if node.is_dir {
                if app.expanded.contains(&node.path) {
                    "▼ "
                } else {
                    "▶ "
                }
            } else {
                "  "
            };
            let (color, bold) = git_status_style(node, app, theme);
            ListItem::new(format!("{}{}{}", indent, arrow, node.name))
                .style(Style::default().fg(color).add_modifier(bold))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .add_modifier(Modifier::BOLD),
    );

    let view_height = area.height.saturating_sub(2).max(1) as usize;
    let mut state = ListState::default();
    if app.tree_independent_scroll {
        // The viewport is driven by `tree_scroll`, decoupled from the cursor.
        // Clamp it so we never scroll past the end, then only highlight the
        // selection when it falls inside the visible window — otherwise the
        // list widget would scroll the viewport back to reveal it.
        let max_scroll = app.nodes.len().saturating_sub(view_height);
        app.tree_scroll = app.tree_scroll.min(max_scroll);
        *state.offset_mut() = app.tree_scroll;
        if !app.nodes.is_empty()
            && app.tree_selected >= app.tree_scroll
            && app.tree_selected < app.tree_scroll + view_height
        {
            state.select(Some(app.tree_selected));
        }
    } else if !app.nodes.is_empty() {
        state.select(Some(app.tree_selected));
    }

    f.render_stateful_widget(list, area, &mut state);

    // Record the geometry of the rendered list (inside the border) and the
    // scroll offset so mouse clicks can be mapped back to node indices.
    app.tree_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    app.tree_offset = state.offset();
    // Keep `tree_scroll` aligned with what was actually rendered so cursor
    // moves and mouse hit-testing share the same offset.
    app.tree_scroll = state.offset();
}

/// Returns the foreground color and modifier for a tree node based on its
/// git status and whether it is a directory. Deleted files get `diff_del`,
/// new files `diff_add`, modified files `accent_alt`, ignored files gray.
fn git_status_style(
    node: &crate::tree::TreeNode,
    app: &App,
    theme: &crate::theme::Theme,
) -> (ratatui::style::Color, Modifier) {
    use ratatui::style::Color;
    let dir_bold = if node.is_dir {
        Modifier::BOLD
    } else {
        Modifier::empty()
    };

    if node.deleted {
        return (theme.diff_del, Modifier::empty());
    }
    if app.git_status_enabled {
        match app.git_status_map.get(&node.path) {
            Some(GitStatus::New) => return (theme.diff_add, dir_bold),
            Some(GitStatus::Modified) => return (theme.accent_alt, dir_bold),
            Some(GitStatus::Deleted) => return (theme.diff_del, dir_bold),
            Some(GitStatus::Ignored) => return (Color::DarkGray, dir_bold),
            None => {}
        }
    }
    if node.is_dir {
        (theme.dir, Modifier::BOLD)
    } else {
        (theme.file, Modifier::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::config::Config;
    use crate::git::GitStatus;
    use crate::theme::Theme;
    use crate::tree::TreeNode;
    use ratatui::style::Color;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn make_node(name: &str, is_dir: bool, deleted: bool) -> TreeNode {
        TreeNode {
            path: PathBuf::from(name),
            name: name.to_string(),
            depth: 0,
            is_dir,
            deleted,
        }
    }

    fn make_app(git_status_enabled: bool, status_map: HashMap<PathBuf, GitStatus>) -> App {
        let cfg = Config {
            git_status: false, // don't fetch real git status
            ..Config::default()
        };
        let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
        app.git_status_enabled = git_status_enabled;
        app.git_status_map = status_map;
        app
    }

    fn default_theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn git_status_deleted_file_uses_diff_del() {
        let node = make_node("gone.rs", false, true);
        let app = make_app(false, HashMap::new());
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().diff_del);
    }

    #[test]
    fn git_status_new_file_uses_diff_add() {
        let node = make_node("new.rs", false, false);
        let mut map = HashMap::new();
        map.insert(PathBuf::from("new.rs"), GitStatus::New);
        let app = make_app(true, map);
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().diff_add);
    }

    #[test]
    fn git_status_modified_file_uses_accent_alt() {
        let node = make_node("mod.rs", false, false);
        let mut map = HashMap::new();
        map.insert(PathBuf::from("mod.rs"), GitStatus::Modified);
        let app = make_app(true, map);
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().accent_alt);
    }

    #[test]
    fn git_status_ignored_file_uses_dark_gray() {
        let node = make_node("ignored.log", false, false);
        let mut map = HashMap::new();
        map.insert(PathBuf::from("ignored.log"), GitStatus::Ignored);
        let app = make_app(true, map);
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, Color::DarkGray);
    }

    #[test]
    fn git_status_regular_file_uses_file_color() {
        let node = make_node("plain.txt", false, false);
        let app = make_app(false, HashMap::new());
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().file);
    }

    #[test]
    fn git_status_regular_dir_uses_dir_color_and_bold() {
        let node = make_node("mydir", true, false);
        let app = make_app(false, HashMap::new());
        let (color, bold) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().dir);
        assert_eq!(bold, Modifier::BOLD);
    }

    #[test]
    fn git_status_deleted_takes_precedence_over_git_status() {
        let node = make_node("gone.rs", false, true);
        let mut map = HashMap::new();
        map.insert(PathBuf::from("gone.rs"), GitStatus::New);
        let app = make_app(true, map);
        let (color, _) = git_status_style(&node, &app, &default_theme());
        // deleted flag takes precedence → diff_del, not diff_add
        assert_eq!(color, default_theme().diff_del);
    }

    #[test]
    fn git_status_enabled_but_path_not_in_map_uses_default() {
        let node = make_node("unknown.rs", false, false);
        let map = HashMap::new(); // empty status map
        let app = make_app(true, map);
        let (color, _) = git_status_style(&node, &app, &default_theme());
        assert_eq!(color, default_theme().file);
    }
}
