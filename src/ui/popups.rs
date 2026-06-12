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
                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{}:{}: ", file.display(), m.line_num),
                        Style::default().fg(theme.accent),
                    ),
                    Span::raw(trimmed),
                ]))
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
        .map(|&i| ListItem::new(picker.names[i]))
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
        Line::from(vec![
            key("  M          "),
            desc("toggle markdown render (md files)"),
        ]),
        Line::from(vec![
            key("  H          "),
            desc("git history of current file"),
        ]),
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
