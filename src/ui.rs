use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus, SearchMode};

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

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

    draw_tree(f, app, horiz[0]);
    draw_content(f, app, horiz[1]);
    draw_statusbar(f, app, vert[1]);

    if app.search.is_some() {
        draw_search(f, app, area);
    }

    if app.show_help {
        draw_help(f, area);
    }
}

fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Tree) && app.search.is_none();
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = format!(
        " {} ",
        app.root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "/".to_string())
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
            let (color, bold) = if node.is_dir {
                (Color::Blue, Modifier::BOLD)
            } else {
                (Color::Reset, Modifier::empty())
            };
            ListItem::new(format!("{}{}{}", indent, arrow, node.name))
                .style(Style::default().fg(color).add_modifier(bold))
        })
        .collect();

    let list = List::new(items).block(block).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if !app.nodes.is_empty() {
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
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Content) && app.search.is_none();
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = app
        .current_file
        .as_ref()
        .and_then(|p| p.strip_prefix(&app.root).ok())
        .map(|rel| format!(" {} ", rel.display()))
        .unwrap_or_else(|| " No file ".into());

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let lines: Vec<Line> = if app.is_markdown && !app.show_raw_markdown {
        app.markdown_lines
            .iter()
            .map(|spans| {
                Line::from(
                    spans
                        .iter()
                        .map(|(s, t)| Span::styled(t.as_str(), *s))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    } else {
        let ln_width = app.content.len().to_string().len().max(1);
        let ln_style = Style::default().fg(Color::DarkGray);
        if !app.highlighted.is_empty() {
            app.highlighted
                .iter()
                .enumerate()
                .map(|(i, regions)| {
                    let mut spans = vec![Span::styled(
                        format!("{:>width$} ", i + 1, width = ln_width),
                        ln_style,
                    )];
                    spans.extend(regions.iter().map(|(s, t)| Span::styled(t.as_str(), *s)));
                    Line::from(spans)
                })
                .collect()
        } else {
            app.content
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    Line::from(vec![
                        Span::styled(format!("{:>width$} ", i + 1, width = ln_width), ln_style),
                        Span::raw(text.as_str()),
                    ])
                })
                .collect()
        }
    };

    let hscroll = if app.word_wrap {
        0
    } else {
        app.content_hscroll as u16
    };
    let mut para = Paragraph::new(lines)
        .block(block)
        .scroll((app.content_scroll as u16, hscroll));
    if app.word_wrap {
        para = para.wrap(Wrap { trim: false });
    }

    f.render_widget(para, area);

    app.content_area = Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
}

fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let hidden_indicator = if app.show_hidden { " [hidden]" } else { "" };
    let md_hint = if app.is_markdown {
        if app.show_raw_markdown {
            "  M render"
        } else {
            "  M raw"
        }
    } else {
        ""
    };
    let wrap_hint = if app.word_wrap {
        "  z no-wrap"
    } else {
        "  z wrap"
    };
    let hscroll_hint = if app.word_wrap {
        ""
    } else {
        "  ←/→ h-scroll  0 reset col"
    };
    let content_hint = format!(
        " j/k scroll  PgUp/PgDn{}  g/G top/bot  Tab panel  q quit{}{}",
        hscroll_hint, md_hint, wrap_hint
    );
    let tree_hint = format!(
        " j/k nav  Enter/l expand  h collapse  / files  f content  Tab panel  q quit  ? help{}",
        hidden_indicator
    );
    let text: &str = if app.search.is_some() {
        " ↑↓ navigate  Enter select  Tab toggle mode  Esc cancel"
    } else {
        match app.focus {
            Focus::Tree => &tree_hint,
            Focus::Content => &content_hint,
        }
    };

    f.render_widget(
        Paragraph::new(text).style(Style::default().bg(Color::DarkGray).fg(Color::White)),
        area,
    );
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
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
        .border_style(Style::default().fg(Color::Yellow));

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

    // Query input
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
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(search.query.as_str()),
            Span::styled("█", Style::default().fg(Color::Yellow)),
            Span::styled(hint, Style::default().fg(Color::DarkGray)),
        ])),
        parts[0],
    );

    // Divider
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize))
            .style(Style::default().fg(Color::DarkGray)),
        parts[1],
    );

    // Results list
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
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::raw(trimmed),
                ]))
            })
            .collect(),
    };

    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    );

    let mut state = ListState::default();
    if search.results_len() > 0 {
        state.select(Some(search.selected));
    }

    f.render_stateful_widget(list, parts[2], &mut state);

    // Record the results-list geometry and scroll offset for mouse mapping.
    app.search_area = parts[2];
    app.search_offset = state.offset();
}

fn draw_help(f: &mut Frame, area: Rect) {
    let popup = centered_rect(52, 80, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Help — ? / Esc to close ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let key = |k: &'static str| {
        Span::styled(
            k,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    };
    let desc = |d: &'static str| Span::styled(d, Style::default().fg(Color::White));
    let section = |s: &'static str| {
        Line::from(vec![Span::styled(
            s,
            Style::default()
                .fg(Color::Cyan)
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
        gap.clone(),
        section("Search popup"),
        Line::from(vec![
            key("  Tab        "),
            desc("switch files ↔ content mode"),
        ]),
        Line::from(vec![key("  Enter      "), desc("open selected result")]),
        Line::from(vec![key("  ↑↓         "), desc("navigate results")]),
        Line::from(vec![key("  Esc        "), desc("close search")]),
    ];

    f.render_widget(Paragraph::new(rows), inner);
}

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
