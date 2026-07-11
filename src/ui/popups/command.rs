//! The Ctrl-P command-palette popup with prefix routing.
//!
//! `draw_command_palette` renders the command palette as a centered overlay: a
//! query input on top and, below it, the fuzzy-filtered list of the picker's
//! commands (built-in plus plugin-contributed) with the selected entry
//! highlighted. It reads the live `CommandPalette` picker
//! state from `App` (query, filtered-and-scored entries, selection, match
//! positions) and is a no-op when the palette is closed. Selection and dispatch
//! are handled elsewhere - this module is purely the visual surface, mirroring
//! the layout of the other list-style pickers (search, history, and theme).
//!
//! **Prefix routing**: when the first query character is a prefix (`/`, `#`,
//! `:`), the palette switches to file search, content search, or go-to-line
//! mode respectively. The title, query bar, and result list adapt to the
//! active route. Categories are shown as a dimmed prefix (`General:`, `View:`,
//! ...) for the commands route. Descriptions (when present) follow the name
//! in dim text. Keybinding labels are right-aligned to form a scannable
//! column. When the query is non-empty, matched characters in the display
//! text (category + name) are highlighted with bold weight.
//!
//! Entries the current app state can't run (per `picker.inapplicability_reasons`)
//! are dimmed entirely and show their reason in place of the description.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

#[cfg(test)]
#[path = "command_test.rs"]
mod tests;

pub(crate) fn draw_command_palette(f: &mut Frame, app: &mut App, area: Rect) {
    // Read route info from the picker, then drop the borrow so helpers can
    // take &mut App for geometry storage.
    let (route, palette_key) = match app.command_palette.as_ref() {
        Some(p) => {
            let key = app
                .keys()
                .command_palette
                .first()
                .map(|b| b.display())
                .unwrap_or_else(|| "Ctrl+P".to_string());
            (p.route, key)
        }
        None => return,
    };

    let theme = app.theme.clone();
    let popup = centered_rect(56, 65, area);
    f.render_widget(Clear, popup);

    let route_label = route.label();
    let title = format!(" {route_label} - {palette_key} ");

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

    // Query bar: show the prefix character when in a routed mode.
    let (prefix_str, active_query) = match app.command_palette.as_ref() {
        Some(p) => {
            let prefix = match p.route {
                crate::command_palette::PaletteRoute::Commands => String::new(),
                r => format!("{}", r.prefix_char()),
            };
            (prefix, p.active_query().to_string())
        }
        None => return,
    };
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{prefix_str}> "),
                Style::default()
                    .fg(theme.accent_alt)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(active_query),
            Span::styled("|", Style::default().fg(theme.accent_alt)),
        ])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("-".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let list_area = parts[2];

    match route {
        crate::command_palette::PaletteRoute::Commands => {
            draw_command_results(f, app, list_area, &theme);
        }
        crate::command_palette::PaletteRoute::Files => {
            draw_file_results(f, app, list_area, &theme);
        }
        crate::command_palette::PaletteRoute::Content => {
            draw_content_results(f, app, list_area, &theme);
        }
        crate::command_palette::PaletteRoute::GotoLine => {
            draw_goto_line_hint(f, list_area, &theme);
        }
    }
}

/// Renders the command list (the default, no-prefix route).
fn draw_command_results(
    f: &mut Frame,
    app: &mut App,
    list_area: Rect,
    theme: &crate::theme::Theme,
) {
    let picker = match app.command_palette.as_ref() {
        Some(p) => p,
        None => return,
    };
    let available_width = list_area.width as usize;

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .enumerate()
        .map(|(pos, &i)| {
            let cmd = &picker.all_commands[i];
            let is_pinned = pos < picker.base_pinned && picker.query.is_empty();
            let prefix_str = if is_pinned { "★ " } else { "  " };

            let mut display_str = String::new();
            if let Some(cat) = &cmd.category {
                display_str.push_str(cat);
                display_str.push_str(": ");
            }
            display_str.push_str(&cmd.name);

            let display_char_count = display_str.chars().count();
            let hit_positions: std::collections::HashSet<usize> = picker
                .match_positions
                .get(pos)
                .map(|positions| {
                    positions
                        .iter()
                        .filter(|&&p| p < display_char_count)
                        .copied()
                        .collect()
                })
                .unwrap_or_default();

            let reason = picker.inapplicability_reasons.get(i).copied().flatten();
            let is_inapplicable = reason.is_some();

            let normal = if is_inapplicable {
                Style::default().fg(theme.dim)
            } else {
                Style::default().fg(theme.text)
            };
            let highlighted = if is_inapplicable {
                Style::default().fg(theme.dim).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text).add_modifier(Modifier::BOLD)
            };
            let dim = Style::default().fg(theme.dim);

            let mut name_spans: Vec<Span> = Vec::new();
            if hit_positions.is_empty() {
                name_spans.push(Span::styled(display_str.clone(), normal));
            } else {
                let mut run = String::new();
                let mut run_hit: Option<bool> = None;
                for (char_idx, ch) in display_str.chars().enumerate() {
                    let is_hit = hit_positions.contains(&char_idx);
                    if run_hit != Some(is_hit) {
                        if !run.is_empty() {
                            let style = if run_hit == Some(true) {
                                highlighted
                            } else {
                                normal
                            };
                            name_spans.push(Span::styled(std::mem::take(&mut run), style));
                        }
                        run_hit = Some(is_hit);
                    }
                    run.push(ch);
                }
                if !run.is_empty() {
                    let style = if run_hit == Some(true) {
                        highlighted
                    } else {
                        normal
                    };
                    name_spans.push(Span::styled(run, style));
                }
            }

            if let Some(reason_str) = reason {
                name_spans.push(Span::styled(" — ".to_string(), dim));
                name_spans.push(Span::styled(reason_str.to_string(), dim));
            } else if let Some(desc) = &cmd.description {
                name_spans.push(Span::styled(" — ".to_string(), dim));
                name_spans.push(Span::styled(desc.to_string(), dim));
            }

            let binding_text = if picker.binding_labels[i].is_empty() {
                String::new()
            } else {
                format!("[{}]", picker.binding_labels[i])
            };

            let left_width: usize = name_spans.iter().map(|s| s.content.len()).sum();
            let binding_width = binding_text.len();
            let pad = if binding_width > 0 {
                available_width.saturating_sub(left_width + binding_width + 2)
            } else {
                0
            };

            let mut spans: Vec<Span> = Vec::with_capacity(3 + name_spans.len());
            spans.push(Span::styled(prefix_str, Style::default()));
            spans.extend(name_spans);
            if binding_width > 0 {
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled(binding_text, dim));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if picker.results_len() > 0 {
        state.select(Some(picker.selected));
    }

    f.render_stateful_widget(list, list_area, &mut state);

    app.command_palette_area = list_area;
    app.command_palette_offset = state.offset();
}

/// Renders file-path results for the `/` (Files) route.
fn draw_file_results(f: &mut Frame, app: &mut App, list_area: Rect, theme: &crate::theme::Theme) {
    let (file_results, selected, total) = match app.command_palette.as_ref() {
        Some(p) => match p.route_search.as_ref() {
            Some(s) => (&s.file_results, s.selected, s.results_len()),
            None => return,
        },
        None => return,
    };

    let items: Vec<ListItem> = file_results
        .iter()
        .map(|p| {
            let display = p.strip_prefix(&app.root).unwrap_or(p);
            ListItem::new(display.to_string_lossy().to_string())
        })
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if total > 0 {
        state.select(Some(selected));
    }

    f.render_stateful_widget(list, list_area, &mut state);

    app.command_palette_area = list_area;
    app.command_palette_offset = state.offset();
}

/// Renders content-match results for the `#` (Content) route.
fn draw_content_results(
    f: &mut Frame,
    app: &mut App,
    list_area: Rect,
    theme: &crate::theme::Theme,
) {
    let (content_results, selected, total) = match app.command_palette.as_ref() {
        Some(p) => match p.route_search.as_ref() {
            Some(s) => (&s.content_results, s.selected, s.results_len()),
            None => return,
        },
        None => return,
    };

    let root = app.root.clone();
    let items: Vec<ListItem> = content_results
        .iter()
        .map(|m| {
            let file = m.path.strip_prefix(&root).unwrap_or(&m.path);
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
        .collect();

    let list =
        List::new(items).highlight_style(theme.selection_style().add_modifier(Modifier::BOLD));

    let mut state = ListState::default();
    if total > 0 {
        state.select(Some(selected));
    }

    f.render_stateful_widget(list, list_area, &mut state);

    app.command_palette_area = list_area;
    app.command_palette_offset = state.offset();
}

/// Renders a hint for the `:` (Go to Line) route, which has no selectable list.
fn draw_goto_line_hint(f: &mut Frame, list_area: Rect, theme: &crate::theme::Theme) {
    let hint = Line::from(vec![
        Span::styled("Enter a line number", Style::default().fg(theme.dim)),
        Span::styled(" (e.g. 42, +5, -3)", Style::default().fg(theme.dim)),
    ]);
    f.render_widget(Paragraph::new(hint), list_area);
}
