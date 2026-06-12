use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{App, Focus};
use crate::git::GitStatus;
use crate::search::SearchMode;

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

    draw_tree(f, app, horiz[0]);
    draw_content(f, app, horiz[1]);
    draw_statusbar(f, app, vert[1]);

    if app.search.is_some() {
        draw_search(f, app, area);
    }

    if app.history.is_some() {
        draw_history(f, app, area);
    }

    if app.theme_picker.is_some() {
        draw_theme(f, app, area);
    }

    if app.show_help {
        draw_help(f, app, area);
    }
}

fn draw_tree(f: &mut Frame, app: &mut App, area: Rect) {
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

fn apply_selection(
    regions: &[(Style, String)],
    col_start: usize,
    col_end: usize,
    sel_bg: Color,
) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    let mut col = 0;
    for (style, text) in regions {
        let chars: Vec<char> = text.chars().collect();
        let span_len = chars.len();
        let before_end = col_start.saturating_sub(col).min(span_len);
        let hl_end = if col_end == usize::MAX {
            span_len
        } else {
            col_end.saturating_sub(col).min(span_len)
        };
        if before_end > 0 {
            result.push(Span::styled(
                chars[..before_end].iter().collect::<String>(),
                *style,
            ));
        }
        if before_end < hl_end {
            result.push(Span::styled(
                chars[before_end..hl_end].iter().collect::<String>(),
                style.bg(sel_bg),
            ));
        }
        if hl_end < span_len {
            result.push(Span::styled(
                chars[hl_end..].iter().collect::<String>(),
                *style,
            ));
        }
        col += span_len;
    }
    result
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let focused = matches!(app.focus, Focus::Content)
        && app.search.is_none()
        && app.history.is_none()
        && app.theme_picker.is_none();
    let border_style = if focused {
        Style::default().fg(app.theme.accent)
    } else {
        Style::default().fg(app.theme.dim)
    };

    let title = if let Some(t) = &app.content_title {
        t.clone()
    } else {
        app.current_file
            .as_ref()
            .and_then(|p| p.strip_prefix(&app.root).ok())
            .map(|rel| format!(" {} ", rel.display()))
            .unwrap_or_else(|| " No file ".into())
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);

    let sel = app.selection.as_ref().map(|s| s.normalized());
    let sel_bg = app.theme.selection_bg;

    let lines: Vec<Line> = if app.is_diff {
        // Diff view: styled spans, no line-number gutter, no selection.
        app.highlighted
            .iter()
            .map(|spans| {
                Line::from(
                    spans
                        .iter()
                        .map(|(s, t)| Span::styled(t.clone(), *s))
                        .collect::<Vec<_>>(),
                )
            })
            .collect()
    } else if app.is_markdown && !app.show_raw_markdown {
        app.markdown_lines
            .iter()
            .enumerate()
            .map(|(i, spans)| {
                let regions_owned: Vec<(Style, String)> =
                    spans.iter().map(|(s, t)| (*s, t.clone())).collect();
                if let Some(((sl, sc), (el, ec))) = sel {
                    if i >= sl && i <= el {
                        let col_start = if i == sl { sc } else { 0 };
                        let col_end = if i == el { ec } else { usize::MAX };
                        Line::from(apply_selection(&regions_owned, col_start, col_end, sel_bg))
                    } else {
                        Line::from(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s))
                                .collect::<Vec<_>>(),
                        )
                    }
                } else {
                    Line::from(
                        regions_owned
                            .iter()
                            .map(|(s, t)| Span::styled(t.clone(), *s))
                            .collect::<Vec<_>>(),
                    )
                }
            })
            .collect()
    } else {
        let ln_width = app.content.len().to_string().len().max(1);
        let ln_style = Style::default().fg(app.theme.dim);
        if !app.highlighted.is_empty() {
            app.highlighted
                .iter()
                .enumerate()
                .map(|(i, regions)| {
                    let mut spans = vec![Span::styled(
                        format!("{:>width$} ", i + 1, width = ln_width),
                        ln_style,
                    )];
                    let regions_owned: Vec<(Style, String)> =
                        regions.iter().map(|(s, t)| (*s, t.clone())).collect();
                    if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
                            spans.extend(apply_selection(
                                &regions_owned,
                                col_start,
                                col_end,
                                sel_bg,
                            ));
                        } else {
                            spans.extend(
                                regions_owned
                                    .iter()
                                    .map(|(s, t)| Span::styled(t.clone(), *s)),
                            );
                        }
                    } else {
                        spans.extend(
                            regions_owned
                                .iter()
                                .map(|(s, t)| Span::styled(t.clone(), *s)),
                        );
                    }
                    Line::from(spans)
                })
                .collect()
        } else {
            app.content
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    let mut spans = vec![Span::styled(
                        format!("{:>width$} ", i + 1, width = ln_width),
                        ln_style,
                    )];
                    if let Some(((sl, sc), (el, ec))) = sel {
                        if i >= sl && i <= el {
                            let col_start = if i == sl { sc } else { 0 };
                            let col_end = if i == el { ec } else { usize::MAX };
                            let region = vec![(Style::default(), text.clone())];
                            spans.extend(apply_selection(&region, col_start, col_end, sel_bg));
                        } else {
                            spans.push(Span::raw(text.clone()));
                        }
                    } else {
                        spans.push(Span::raw(text.clone()));
                    }
                    Line::from(spans)
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
        " j/k scroll  PgUp/PgDn{}  g/G top/bot  H history  Tab panel  q quit{}{}",
        hscroll_hint, md_hint, wrap_hint
    );
    let git_indicator = if app.git_mode {
        if app.git_mode_flat {
            " [git:flat]"
        } else {
            " [git]"
        }
    } else {
        ""
    };
    let tree_hint = format!(
        " j/k nav  Enter/l expand  h collapse  / files  f content  t theme  Tab panel  q quit  ? help{}{}",
        hidden_indicator, git_indicator
    );
    let text: &str = if app.theme_picker.is_some() {
        " ↑↓ navigate  type to filter  Enter apply theme  Esc cancel"
    } else if app.history.is_some() {
        " ↑↓ navigate  type to filter  Enter show diff  Esc cancel"
    } else if app.search.is_some() {
        " ↑↓ navigate  Enter select  Tab toggle mode  Esc cancel"
    } else {
        match app.focus {
            Focus::Tree => &tree_hint,
            Focus::Content => &content_hint,
        }
    };

    f.render_widget(
        Paragraph::new(text).style(
            Style::default()
                .bg(app.theme.selection_bg)
                .fg(app.theme.text),
        ),
        area,
    );
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect) {
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
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(search.query.as_str()),
            Span::styled("█", Style::default().fg(theme.accent_alt)),
            Span::styled(hint, Style::default().fg(theme.dim)),
        ])),
        parts[0],
    );

    // Divider
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
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

    // Record the results-list geometry and scroll offset for mouse mapping.
    app.search_area = parts[2];
    app.search_offset = state.offset();
}

fn draw_history(f: &mut Frame, app: &mut App, area: Rect) {
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

    // Query input
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

    // Divider
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    // Commit list
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

fn draw_theme(f: &mut Frame, app: &mut App, area: Rect) {
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

    // Query input
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

    // Divider
    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    // Preset list
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

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
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
