//! Diff information and action toolbar for the content pane.
//!
//! When the content pane displays a diff (working-tree diff, revision diff, or
//! commit diff), this module renders a 1-line header bar above the diff content.
//! It clarifies the active diff mode (Unstaged, Staged, or All), displays the
//! number of @@ hunks detected in the file, indicates the view layout (Unified
//! or Side-by-side), and surfaces direct key shortcuts for cycling diff mode (`s`),
//! toggling side-by-side view (`S`), and jumping between hunks (`n`/`N`).
//! The area is recorded on `App` so mouse clicks can toggle staged/unstaged diffs.
//!
//! Owned public items:
//! - `draw_diff_info_bar`: renders the 1-line toolbar at the top of the diff pane.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::App;

/// Draws a compact 1-line diff information bar at the top of the diff pane.
pub(crate) fn draw_diff_info_bar(f: &mut Frame, app: &App, area: Rect) {
    if area.height == 0 || area.width < 10 {
        return;
    }

    let theme = &app.theme;
    let base_style = Style::default().bg(theme.selection_bg).fg(theme.text);
    let badge_style = base_style.fg(theme.accent).add_modifier(Modifier::BOLD);
    let dim_style = base_style.fg(theme.dim);

    let mode_label = format!(" Diff: {} ", app.diff_mode.label());
    let hunks = app.diff_hunk_rows().len();
    let hunk_str = if hunks == 1 {
        " 1 hunk ".to_string()
    } else {
        format!(" {hunks} hunks ")
    };
    let layout_str = if app.diff_side_by_side {
        " [side-by-side] "
    } else {
        " [unified] "
    };

    let mut spans = vec![
        Span::styled(mode_label, badge_style),
        Span::styled(hunk_str, dim_style),
        Span::styled(layout_str, base_style),
    ];

    let staged_key = app.keys().label_for_action("toggle_diff_staged");
    let sxs_key = app.keys().label_for_action("toggle_diff_side_by_side");
    let next_key = app.keys().label_for_action("diff_hunk_next");
    let prev_key = app.keys().label_for_action("diff_hunk_prev");

    let total_prefix_w: usize = spans.iter().map(Span::width).sum();
    let avail = (area.width as usize).saturating_sub(total_prefix_w);

    let hint_str = if !staged_key.is_empty() && !sxs_key.is_empty() && avail >= 38 {
        format!(" [{staged_key}] stage  [{sxs_key}] split  [{next_key}/{prev_key}] hunks ")
    } else if !staged_key.is_empty() && !sxs_key.is_empty() && avail >= 22 {
        format!(" [{staged_key}] stage  [{sxs_key}] split ")
    } else if !staged_key.is_empty() && avail >= 12 {
        format!(" [{staged_key}] stage ")
    } else {
        String::new()
    };

    if !hint_str.is_empty() {
        let hint_w = hint_str.len();
        let gap = avail.saturating_sub(hint_w);
        if gap > 0 {
            spans.push(Span::styled(" ".repeat(gap), base_style));
        }
        spans.push(Span::styled(hint_str, dim_style));
    }

    f.render_widget(Paragraph::new(Line::from(spans)).style(base_style), area);
}

#[cfg(test)]
#[path = "diff_header_test.rs"]
mod tests;
