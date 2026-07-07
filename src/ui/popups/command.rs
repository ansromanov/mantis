//! The Ctrl-P command-palette popup.
//!
//! `draw_command_palette` renders the command palette as a centered overlay: a
//! query input on top and, below it, the fuzzy-filtered list of `COMMANDS` with
//! the selected entry highlighted. It reads the live `CommandPalette` picker
//! state from `App` (query, filtered-and-scored entries, selection, match
//! positions) and is a no-op when the palette is closed. Selection and dispatch
//! are handled elsewhere - this module is purely the visual surface, mirroring
//! the layout of the other list-style pickers (search, history, and theme).
//!
//! Categories are shown as a dimmed prefix (`General:`, `View:`, ...).
//! Descriptions (when present) follow the name in dim text.
//! Keybinding labels are right-aligned to form a scannable column.
//! When the query is non-empty, matched characters in the display text (category
//! + name) are highlighted with bold weight.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::command_palette::COMMANDS;

use super::util::centered_rect;

#[cfg(test)]
#[path = "command_test.rs"]
mod tests;

pub(crate) fn draw_command_palette(f: &mut Frame, app: &mut App, area: Rect) {
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

    let list_area = parts[2];
    let available_width = list_area.width as usize;

    let items: Vec<ListItem> = picker
        .filtered
        .iter()
        .enumerate()
        .map(|(pos, &i)| {
            let cmd = &COMMANDS[i];
            let is_pinned = pos < picker.base_pinned && picker.query.is_empty();
            let prefix_str = if is_pinned { "★ " } else { "  " };

            // Build the display text (category + name) for match highlighting.
            let mut display_str = String::new();
            if let Some(cat) = cmd.category {
                display_str.push_str(cat);
                display_str.push_str(": ");
            }
            display_str.push_str(cmd.name);

            // Extract match positions that fall within the display text.
            // Positions are *character* indices into the full haystack (as
            // returned by `fuzzy_indices`, which matches over a `Vec<char>`,
            // not bytes). Positions beyond the char count of `display_str`
            // are in the binding portion of the haystack and are ignored.
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

            // Build name/category spans with fuzzy-match highlighting.
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
                // Walk by char (not byte) so multi-byte UTF-8 characters can't
                // land a slice on a non-char boundary.
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

            // Description or inapplicable reason (dimmed, separated from name by " — ").
            if let Some(reason_str) = reason {
                name_spans.push(Span::styled(" — ".to_string(), dim));
                name_spans.push(Span::styled(reason_str.to_string(), dim));
            } else if let Some(desc) = cmd.description {
                name_spans.push(Span::styled(" — ".to_string(), dim));
                name_spans.push(Span::styled(desc.to_string(), dim));
            }

            // Binding label (right-aligned).
            let binding_text = if picker.binding_labels[i].is_empty() {
                String::new()
            } else {
                format!("[{}]", picker.binding_labels[i])
            };

            // Compute left-part width for right-alignment padding.
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
