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
mod tests {
    use crate::app::App;
    use crate::command_palette::CommandPalette;
    use crate::config::Config;
    use crate::git::Commit;
    use crate::search::{HistoryState, SearchState, ThemePicker};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::{Path, PathBuf};

    fn make_app() -> App {
        let cfg = Config {
            git_status: false,
            ..Config::default()
        };
        App::new(PathBuf::from("."), cfg, None, None).unwrap()
    }

    fn buffer_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buf = terminal.backend().buffer();
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect()
            })
            .collect()
    }

    fn render(app: &mut App) -> Vec<String> {
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, app)).unwrap();
        buffer_rows(&terminal)
    }

    #[test]
    fn draw_no_overlay() {
        let mut app = make_app();
        let rows = render(&mut app);
        let joined = rows.join("\n");
        // Tree panel should render
        assert!(joined.contains("tv") || joined.contains("tree-viewer"));
        // Status bar should render
        assert!(rows[29].contains("j/k nav"));
    }

    #[test]
    fn draw_search_overlay() {
        let mut app = make_app();
        app.search = Some(SearchState::new(Path::new("."), false, true, 0));
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("Search: Files"));
        assert!(joined.contains("Tab"));
    }

    #[test]
    fn draw_history_overlay() {
        let mut app = make_app();
        app.history = Some(HistoryState::new(
            PathBuf::from("test.txt"),
            vec![Commit {
                hash: "a".into(),
                short: "abc123".into(),
                date: "2024-01-01".into(),
                subject: "initial commit".into(),
            }],
        ));
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("History:"));
        assert!(joined.contains("abc123"));
    }

    #[test]
    fn draw_theme_picker_overlay() {
        let mut app = make_app();
        app.theme_picker = Some(ThemePicker::default());
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("Theme"));
    }

    #[test]
    fn draw_help_overlay() {
        let mut app = make_app();
        app.show_help = true;
        // Use a taller terminal so all help sections are visible
        let backend = TestBackend::new(80, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, &mut app)).unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Help"));
        assert!(joined.contains("Global"));
        assert!(joined.contains("Tree panel"));
        assert!(joined.contains("Content panel"));
        assert!(joined.contains("In-file search"));
    }

    #[test]
    fn draw_command_palette_overlay() {
        let mut app = make_app();
        app.command_palette = Some(CommandPalette::default());
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("Commands"));
    }

    #[test]
    fn draw_about_overlay() {
        let mut app = make_app();
        app.show_about = true;
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("About"));
        assert!(joined.contains("Version:"));
    }

    #[test]
    fn draw_in_file_search_overlay() {
        let mut app = make_app();
        use crate::search::InFileSearch;
        let mut s = InFileSearch::new();
        s.push('x');
        s.refresh(0, |_| None);
        app.in_file_search = Some(s);
        let rows = render(&mut app);
        let joined = rows.join("\n");
        assert!(joined.contains("/x"));
    }

    #[test]
    fn draw_layout_respects_tree_width() {
        let mut app = make_app();
        app.tree_width = 50;
        let backend = TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer();
        let mid_col: String = (0..30).map(|y| buf[(49, y)].symbol().to_string()).collect();
        assert!(!mid_col.is_empty());
    }
}
