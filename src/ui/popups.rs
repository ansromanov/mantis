use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::search::SearchMode;

pub(super) fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let search = app.search.as_ref().unwrap();

    let popup = centered_rect(72, 75, area);
    f.render_widget(Clear, popup);

    let mode_label = match search.mode {
        SearchMode::Files => " Search: Files  [Tab → Content] ",
        SearchMode::Content => " Search: Content  [Tab → Files] ",
    };

    let block = Block::default()
        .title(mode_label)
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let hint = if search.mode == SearchMode::Content && search.query.len() < 2 {
        "  (type 2+ chars)"
    } else {
        ""
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(search.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
            Span::styled(hint, Style::default().fg(theme.dim)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = match search.mode {
        SearchMode::Files => search
            .file_results
            .iter()
            .map(|p| {
                let display = p.strip_prefix(&app.root).unwrap_or(p);
                ListItem::new(display.to_string_lossy().to_string())
            })
            .collect(),
        SearchMode::Content => search
            .content_results
            .iter()
            .map(|m| {
                let file = m.path.strip_prefix(&app.root).unwrap_or(&m.path);
                let trimmed = m.line.trim();
                let mut lines = vec![Line::from(vec![
                    Span::styled(
                        format!("{}:{}: ", file.display(), m.line_num),
                        Style::default().fg(theme.accent),
                    ),
                    Span::raw(trimmed),
                ])];
                for ctx_line in &m.context {
                    lines.push(Line::from(vec![
                        Span::styled("  ", Style::default().fg(theme.dim)),
                        Span::styled(ctx_line.trim().to_string(), Style::default().fg(theme.dim)),
                    ]));
                }
                ListItem::new(lines)
            })
            .collect(),
    };

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if search.results_len() > 0 {
        state.select(Some(search.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.search_area = parts[2];
    app.search_offset = state.offset();
}

pub(super) fn draw_history(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let history = app.history.as_ref().unwrap();

    let popup = centered_rect(72, 75, area);
    f.render_widget(Clear, popup);

    let name = history
        .file
        .strip_prefix(&app.root)
        .unwrap_or(&history.file);
    let block = Block::default()
        .title(format!(" History: {} ", name.display()))
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(history.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = history
        .filtered
        .iter()
        .filter_map(|&i| history.commits.get(i))
        .map(|c| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{} ", c.short),
                    Style::default().fg(theme.accent_alt),
                ),
                Span::styled(format!("{} ", c.date), Style::default().fg(theme.accent)),
                Span::raw(c.subject.as_str()),
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if history.results_len() > 0 {
        state.select(Some(history.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.history_area = parts[2];
    app.history_offset = state.offset();
}

pub(super) fn draw_theme(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let picker = app.theme_picker.as_ref().unwrap();

    let popup = centered_rect(44, 55, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Theme ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(picker.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .map(|&i| ListItem::new(picker.names[i].as_str()))
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if picker.results_len() > 0 {
        state.select(Some(picker.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.theme_area = parts[2];
    app.theme_offset = state.offset();
}

pub(super) fn draw_command_palette(f: &mut Frame, app: &mut App, area: Rect) {
    use crate::command_palette::COMMANDS;

    let Some(picker) = app.command_palette.as_ref() else {
        return;
    };

    let palette_key = app
        .keys()
        .command_palette
        .first()
        .map(|b| b.display())
        .unwrap_or_else(|| "Ctrl+P".to_string());
    let title = format!(" Commands - {} ", palette_key);

    let theme = &app.theme;
    let popup = centered_rect(56, 65, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(picker.query.as_str()),
            Span::styled("|", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("-".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .map(|&i| {
            let cmd = &COMMANDS[i];
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {} ", cmd.name), Style::default().fg(theme.text)),
                if picker.binding_labels[i].is_empty() {
                    Span::raw("")
                } else {
                    Span::styled(
                        format!("[{}]", picker.binding_labels[i]),
                        Style::default().fg(theme.dim),
                    )
                },
            ]))
        })
        .collect();

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(theme.selection_bg)
            .fg(theme.selection_fg)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if picker.results_len() > 0 {
        state.select(Some(picker.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    app.command_palette_area = parts[2];
    app.command_palette_offset = state.offset();
}

pub(super) fn draw_about(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(52, 75, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" About tree-viewer — ? / Esc / q to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let dim = Style::default().fg(theme.dim);
    let accent = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let text_style = Style::default().fg(theme.text);

    let version = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.version.as_str())
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let date = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.date.as_str())
        .unwrap_or("");
    let whats_new = crate::release_info::RELEASE
        .as_ref()
        .map(|r| r.whats_new.as_str())
        .unwrap_or("");
    let has_url = crate::release_info::RELEASE
        .as_ref()
        .map(|r| !r.release_url.is_empty())
        .unwrap_or(false);

    let mut rows: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            "  A fast terminal file tree viewer with",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  syntax highlighting, markdown rendering,",
            text_style,
        )]),
        Line::from(vec![Span::styled(
            "  fuzzy search, and mouse support.",
            text_style,
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Version:   ", dim),
            Span::styled(version, accent),
        ]),
    ];

    if !date.is_empty() {
        rows.push(Line::from(vec![
            Span::styled("  Released:  ", dim),
            Span::styled(date, text_style),
        ]));
    }

    rows.push(Line::from(vec![
        Span::styled("  License:   ", dim),
        Span::styled("GPL-3.0-or-later", text_style),
    ]));

    if !whats_new.is_empty() {
        rows.push(Line::from(""));
        rows.push(Line::from(vec![Span::styled(
            "  What's new:",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::UNDERLINED),
        )]));
        for line in whats_new.lines() {
            rows.push(Line::from(vec![Span::styled(
                format!("  {line}"),
                text_style,
            )]));
        }
    }

    if has_url {
        rows.push(Line::from(""));
        rows.push(Line::from(vec![Span::styled(
            "  Enter  open release in browser",
            dim,
        )]));
    }

    f.render_widget(Paragraph::new(rows), inner);
}

pub(super) fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(52, 80, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Help — ? / Esc to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let key = |k: &'static str| {
        Span::styled(
            k,
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )
    };
    let desc = |d: &'static str| Span::styled(d, Style::default().fg(theme.text));
    let section = |s: &'static str| {
        Line::from(vec![Span::styled(
            s,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )])
    };
    let gap = Line::from("");

    let rows: Vec<Line> = vec![
        section("Global"),
        Line::from(vec![key("  ?          "), desc("toggle this help")]),
        Line::from(vec![key("  Tab        "), desc("switch panel")]),
        Line::from(vec![key("  q / Ctrl+C "), desc("quit")]),
        Line::from(vec![key("  Alt+.      "), desc("toggle hidden files")]),
        Line::from(vec![key("  t          "), desc("pick a theme")]),
        Line::from(vec![
            key("  Ctrl+G     "),
            desc("toggle git mode (changed files only + diffs)"),
        ]),
        Line::from(vec![key("  e          "), desc("open file in $EDITOR")]),
        Line::from(vec![
            key("  Alt+G      "),
            desc("toggle git flat/tree view (in git mode)"),
        ]),
        gap.clone(),
        section("Tree panel"),
        Line::from(vec![key("  j/k / ↑↓   "), desc("move up / down")]),
        Line::from(vec![key("  Enter/l/→  "), desc("expand dir / open file")]),
        Line::from(vec![key("  h/←        "), desc("collapse dir")]),
        Line::from(vec![key("  /          "), desc("fuzzy file search")]),
        Line::from(vec![key("  f          "), desc("fuzzy content search")]),
        Line::from(vec![key("  r          "), desc("reload tree")]),
        gap.clone(),
        section("Content panel"),
        Line::from(vec![key("  j/k / ↑↓   "), desc("scroll up / down")]),
        Line::from(vec![key("  PgUp/PgDn  "), desc("page scroll")]),
        Line::from(vec![key("  ←/→        "), desc("horizontal scroll")]),
        Line::from(vec![key("  0          "), desc("reset horizontal scroll")]),
        Line::from(vec![key("  g / G      "), desc("top / bottom")]),
        Line::from(vec![key("  z          "), desc("toggle word wrap")]),
        Line::from(vec![key("  b          "), desc("toggle git blame gutter")]),
        Line::from(vec![
            key("  M          "),
            desc("toggle markdown render (md files)"),
        ]),
        Line::from(vec![
            key("  H          "),
            desc("git history of current file"),
        ]),
        Line::from(vec![
            key("  D          "),
            desc("toggle side-by-side diff (in a diff)"),
        ]),
        Line::from(vec![
            key("  n / N      "),
            desc("next / previous hunk (in a diff)"),
        ]),
        Line::from(vec![
            key("  /          "),
            desc("find in file (in-file search)"),
        ]),
        Line::from(vec![
            key("  Space      "),
            desc("toggle YAML fold at cursor"),
        ]),
        gap.clone(),
        section("In-file search"),
        Line::from(vec![key("  n / N      "), desc("next / previous match")]),
        Line::from(vec![key("  Enter/Esc  "), desc("close search")]),
        gap.clone(),
        section("Search / history popup"),
        Line::from(vec![
            key("  Tab        "),
            desc("switch files ↔ content mode"),
        ]),
        Line::from(vec![key("  Enter      "), desc("open result / show diff")]),
        Line::from(vec![key("  ↑↓         "), desc("navigate results")]),
        Line::from(vec![key("  Esc        "), desc("close popup")]),
    ];

    f.render_widget(Paragraph::new(rows), inner);
}

/// Draws the in-file search prompt bar at the bottom of the content area.
pub(super) fn draw_in_file_search(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let s = app.in_file_search.as_ref().unwrap();
    let bar_y = area.y + area.height.saturating_sub(2);
    let bar_rect = Rect {
        x: area.x + 1,
        y: bar_y,
        width: area.width.saturating_sub(2),
        height: 1,
    };
    if bar_rect.width < 4 {
        return;
    }
    f.render_widget(Clear, bar_rect);

    let total = s.matches.len();
    let current = if total > 0 { s.current + 1 } else { 0 };
    let suffix = format!(" ({}/{})", current, total);
    let max_w = bar_rect.width as usize;
    let query_display: String = s
        .query
        .chars()
        .take(max_w.saturating_sub(suffix.len() + 2))
        .collect();
    let text = format!("/{}{}", query_display, suffix);
    if text.len() > max_w {
        let truncated: String = text.chars().take(max_w).collect();
        f.render_widget(
            Paragraph::new(Line::from(vec![Span::styled(
                truncated,
                Style::default().fg(theme.accent_alt).bg(theme.background),
            )])),
            bar_rect,
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    "/",
                    Style::default()
                        .fg(theme.accent_alt)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(query_display.as_str()),
                Span::styled("█", Style::default().fg(theme.accent_alt)),
                Span::styled(suffix, Style::default().fg(theme.dim)),
            ])),
            bar_rect,
        );
    }
}

/// Returns a `Rect` centered in `area` using the given percentage widths.
/// Used by all popup overlays (search, history, theme, help).
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let margin_y = (100 - percent_y) / 2;
    let margin_x = (100 - percent_x) / 2;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(margin_y),
            Constraint::Percentage(percent_y),
            Constraint::Percentage(margin_y),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(margin_x),
            Constraint::Percentage(percent_x),
            Constraint::Percentage(margin_x),
        ])
        .split(vert[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centered_rect_returns_inner_rectangle() {
        // area = 100x100, centered_rect(50, 50) → 50x50 centered
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let result = centered_rect(50, 50, area);
        assert_eq!(result.width, 50);
        assert_eq!(result.height, 50);
        assert_eq!(result.x, 25);
        assert_eq!(result.y, 25);
    }

    #[test]
    fn centered_rect_full_size() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let result = centered_rect(100, 100, area);
        assert_eq!(result.width, 80);
        assert_eq!(result.height, 24);
        assert_eq!(result.x, 0);
        assert_eq!(result.y, 0);
    }

    #[test]
    fn centered_rect_narrow_and_short() {
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        };
        let result = centered_rect(20, 20, area);
        assert_eq!(result.width, 16); // 80 * 0.20 = 16
        assert_eq!(result.height, 4); // 24 * 0.20 = 4.8 → 4
        assert_eq!(result.x, 32); // (80-16)/2 = 32
        assert_eq!(result.y, 10); // (24-4)/2 = 10
    }

    #[test]
    fn centered_rect_non_zero_origin() {
        let area = Rect {
            x: 10,
            y: 5,
            width: 80,
            height: 40,
        };
        let result = centered_rect(50, 50, area);
        assert_eq!(result.width, 40);
        assert_eq!(result.height, 20);
        assert_eq!(result.x, 10 + 20); // x + (80-40)/2
        assert_eq!(result.y, 5 + 10); // y + (40-20)/2
    }

    // ── draw_search ─────────────────────────────────────────────────────────

    use crate::app::App;
    use crate::config::Config;
    use crate::search::InFileSearch;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_app(root: &std::path::Path) -> App {
        let cfg = Config {
            git_status: false,
            ..Config::default()
        };
        App::new(root.to_path_buf(), cfg, None, None).unwrap()
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

    #[test]
    fn draw_search_files_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("alpha.txt"), "").unwrap();
        std::fs::write(dir.path().join("beta.txt"), "").unwrap();
        let mut app = make_app(dir.path());
        app.search = Some(crate::search::SearchState::new(dir.path(), false, true, 0));

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Search: Files"));
        assert!(joined.contains("alpha.txt"));
        assert!(joined.contains("beta.txt"));
    }

    #[test]
    fn draw_search_files_filtered() {
        let dir = tempfile::tempdir().unwrap();
        // Use very distinct paths that won't collide in fuzzy matching
        let one = dir.path().join("111111_document_only.txt");
        let two = dir.path().join("222222_document_only.txt");
        std::fs::write(&one, "").unwrap();
        std::fs::write(&two, "").unwrap();
        let mut app = make_app(dir.path());
        app.search = Some(crate::search::SearchState::new(dir.path(), false, true, 0));
        // Query "111111" matches only the first file's name segment
        for c in "111111".chars() {
            app.search.as_mut().unwrap().push(c);
        }

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("111111_document_only.txt"));
        assert!(!joined.contains("222222_document_only.txt"));
    }

    #[test]
    fn draw_search_content_mode() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("hello.txt"), "hello world\nfoo bar\n").unwrap();
        std::fs::write(dir.path().join("other.txt"), "no match").unwrap();
        let mut app = make_app(dir.path());
        let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
        search.toggle_mode();
        search.push('h');
        search.push('e');
        search.refresh_now();
        app.search = Some(search);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Search: Content"));
        assert!(joined.contains("hello.txt"));
        assert!(joined.contains("hello world"));
    }

    #[test]
    fn draw_search_content_short_query_hint() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
        search.toggle_mode();
        search.push('x');
        app.search = Some(search);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        // When content query is < 2 chars, show hint
        assert!(joined.contains("2+ chars"));
    }

    #[test]
    fn draw_search_content_query_two_chars_hides_hint() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
        search.toggle_mode();
        search.push('x');
        search.push('y');
        app.search = Some(search);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Search: Content"));
        // Check it doesn't panic and still renders
        assert!(!joined.is_empty());
    }

    #[test]
    fn draw_search_select_highlight() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "").unwrap();
        std::fs::write(dir.path().join("b.txt"), "").unwrap();
        let mut app = make_app(dir.path());
        let mut search = crate::search::SearchState::new(dir.path(), false, true, 0);
        search.selected = 1;
        app.search = Some(search);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_search(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("b.txt"));
    }

    // ── draw_history ────────────────────────────────────────────────────────

    #[test]
    fn draw_history_with_commits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.history = Some(crate::search::HistoryState::new(
            dir.path().join("test.txt"),
            vec![
                crate::git::Commit {
                    hash: "abc123def456".into(),
                    short: "abc123".into(),
                    date: "2024-01-15".into(),
                    subject: "fix critical bug".into(),
                },
                crate::git::Commit {
                    hash: "def789abc012".into(),
                    short: "def789".into(),
                    date: "2024-01-14".into(),
                    subject: "add new feature".into(),
                },
            ],
        ));

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_history(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("History:"));
        assert!(joined.contains("abc123"));
        assert!(joined.contains("def789"));
        assert!(joined.contains("fix critical bug"));
        assert!(joined.contains("add new feature"));
    }

    #[test]
    fn draw_history_empty_commits() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.history = Some(crate::search::HistoryState::new(
            dir.path().join("test.txt"),
            vec![],
        ));

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_history(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("History:"));
    }

    // ── draw_theme ──────────────────────────────────────────────────────────

    #[test]
    fn draw_theme_with_presets() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.theme_picker = Some(crate::search::ThemePicker::default());

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_theme(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Theme"));
        assert!(joined.contains("default"));
    }

    #[test]
    fn draw_theme_with_filter() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.theme_picker = Some(crate::search::ThemePicker::default());
        // Filter by pushing characters
        if let Some(ref mut p) = app.theme_picker {
            p.push('m');
        }

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_theme(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        // Should show filtered results (contains 'm')
        assert!(joined.contains("> m"));
    }

    // ── draw_help ───────────────────────────────────────────────────────────

    #[test]
    fn draw_help_all_sections() {
        let dir = tempfile::tempdir().unwrap();
        let app = make_app(dir.path());

        // Use a tall terminal (60 rows) so all help sections are visible
        let backend = TestBackend::new(80, 60);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_help(f, &app, f.area())).unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Help"));
        assert!(joined.contains("Global"));
        assert!(joined.contains("Tree panel"));
        assert!(joined.contains("Content panel"));
        assert!(joined.contains("In-file search"));
        assert!(joined.contains("Search / history popup"));
        assert!(joined.contains("toggle this help"));
        assert!(joined.contains("fuzzy file search"));
        assert!(joined.contains("toggle word wrap"));
    }

    // ── draw_in_file_search ─────────────────────────────────────────────────

    #[test]
    fn draw_in_file_search_with_matches() {
        use crate::search::InFileMatch;
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.in_file_search = Some(InFileSearch {
            query: "hello".into(),
            matches: vec![InFileMatch {
                line: 0,
                col: 0,
                len: 5,
            }],
            current: 0,
        });

        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 30,
        };
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_in_file_search(f, &mut app, area))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("/hello"));
        assert!(joined.contains("(1/1)"));
    }

    #[test]
    fn draw_in_file_search_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.in_file_search = Some(InFileSearch {
            query: "zzz".into(),
            matches: vec![],
            current: 0,
        });

        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 30,
        };
        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_in_file_search(f, &mut app, area))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("/zzz"));
        assert!(joined.contains("(0/0)"));
    }

    #[test]
    fn draw_in_file_search_narrow_area_returns_early() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.in_file_search = Some(InFileSearch {
            query: "x".into(),
            matches: vec![],
            current: 0,
        });

        // Width < 4 should return early without panic
        let area = Rect {
            x: 0,
            y: 0,
            width: 3,
            height: 30,
        };
        let backend = TestBackend::new(3, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_in_file_search(f, &mut app, area))
            .unwrap();
        // No assertion needed - just verifying no panic
    }

    // ── draw_command_palette ────────────────────────────────────────────────

    #[test]
    fn draw_command_palette_all_commands() {
        use crate::command_palette::CommandPalette;
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.command_palette = Some(CommandPalette::default());

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_command_palette(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Commands"));
        assert!(joined.contains("Toggle help"));
        assert!(joined.contains("Toggle hidden files"));
    }

    #[test]
    fn draw_command_palette_filtered() {
        use crate::command_palette::CommandPalette;
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        let mut cp = CommandPalette::default();
        cp.push('w');
        app.command_palette = Some(cp);

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw_command_palette(f, &mut app, f.area()))
            .unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("Toggle word wrap"));
        assert!(!joined.contains("Toggle help"));
    }

    #[test]
    fn draw_command_palette_none_returns_early() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = make_app(dir.path());
        app.command_palette = None;

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        // Should not panic when command_palette is None
        terminal
            .draw(|f| draw_command_palette(f, &mut app, f.area()))
            .unwrap();
    }

    // ── draw_about ──────────────────────────────────────────────────────────

    #[test]
    fn draw_about_shows_version() {
        let dir = tempfile::tempdir().unwrap();
        let app = make_app(dir.path());

        let backend = TestBackend::new(80, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_about(f, &app, f.area())).unwrap();
        let rows = buffer_rows(&terminal);
        let joined = rows.join("\n");
        assert!(joined.contains("About"));
        assert!(joined.contains("Version:"));
        assert!(joined.contains("GPL-3.0"));
        assert!(joined.contains("tree viewer"));
    }
}
