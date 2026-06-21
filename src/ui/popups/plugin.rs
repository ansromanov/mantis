//! The plugin-manager popup.
//!
//! `draw_plugin_picker` renders the plugin list as a centered overlay showing
//! each registered plugin's name and running state (`[✓]` / `[ ]`). The user
//! navigates with Up/Down and toggles a plugin with Space or Enter; Esc closes
//! the overlay without any extra action. Toggling immediately spawns or kills
//! the plugin subprocess and persists the `enabled` flag back to `tv.toml`.
//! State lives in `App::plugin_picker`; this module only draws it. A no-op
//! when the picker is `None`.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;
use crate::plugin::PluginKind;

use super::util::centered_rect;

pub(crate) fn draw_plugin_picker(f: &mut Frame, app: &mut App, area: Rect) {
    let Some(picker) = app.plugin_picker.as_ref() else {
        return;
    };
    let theme = &app.theme;

    let popup = centered_rect(44, 55, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Plugins ")
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
        Paragraph::new(Line::from(vec![Span::styled(
            "Space/Enter: toggle  Esc: close",
            Style::default().fg(theme.dim),
        )])),
        parts[0],
    );

    f.render_widget(
        Paragraph::new("─".repeat(inner.width as usize)).style(Style::default().fg(theme.dim)),
        parts[1],
    );

    let items: Vec<ListItem> = picker
        .entries
        .iter()
        .map(|(name, running, kind)| {
            let (marker, marker_style) = if *running {
                ("[✓] ", Style::default().fg(theme.diff_add))
            } else {
                ("[ ] ", Style::default().fg(theme.dim))
            };
            let kind_badge = match kind {
                PluginKind::Syntax => " [syntax]",
                PluginKind::Process => "",
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, marker_style),
                Span::raw(name.as_str()),
                Span::styled(kind_badge, Style::default().fg(theme.dim)),
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

    app.plugin_picker_area = parts[2];
    app.plugin_picker_offset = state.offset();
}
