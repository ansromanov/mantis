//! The hint row: one line of clickable key hints for what you are doing now.
//!
//! Below the status bar (`[ui] hints = true`, the default) the row shows up to
//! five actions that make sense in the current context (the focused pane, a
//! diff, rendered markdown...), followed by help and "more…", which opens the
//! context menu. Labels come from the live keymap, so remapped keys show up,
//! and an action is listed only when it applies and has a key in the focused
//! pane. A click replays that key (see `App::handle_mouse`). While a popup is
//! open the row stays empty: the status bar already shows that popup's keys.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus};
use crate::config::BindingScope;

/// Candidates for the tree pane, in priority order.
const TREE_HINTS: &[(&str, &str)] = &[
    ("tree_expand", "open"),
    ("search_files", "filter"),
    ("command_palette", "find anything"),
    ("search_content", "search text"),
    ("git_mode_toggle", "changes"),
];

/// Candidates for the content pane with a file open, in priority order.
/// Context-specific actions come first so they win the limited space.
const CONTENT_HINTS: &[(&str, &str)] = &[
    ("diff_hunk_next", "next hunk"),
    ("toggle_diff_staged", "staged"),
    ("toggle_diff_side_by_side", "side by side"),
    ("toggle_raw_markdown", "raw/rendered"),
    ("search_files", "find in file"),
    ("goto_line", "go to line"),
    ("toggle_wrap", "wrap"),
    ("copy_line", "copy line"),
];

/// Candidates for the content pane when nothing is open yet.
const EMPTY_CONTENT_HINTS: &[(&str, &str)] = &[
    ("find_files", "find file"),
    ("command_palette", "find anything"),
];

/// Always shown last.
const TAIL_HINTS: &[(&str, &str)] = &[("help", "help"), ("context_menu", "more…")];

const MAX_CONTEXT_HINTS: usize = 5;

/// One hint: the action it triggers, the key to show, and a short label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Hint {
    pub action: &'static str,
    pub key: String,
    pub label: &'static str,
}

/// The hints for the current state, or none while a popup is open.
pub(crate) fn hint_items(app: &App) -> Vec<Hint> {
    if app.top_overlay().is_some() {
        return Vec::new();
    }
    let scope = match app.focus {
        Focus::Tree => BindingScope::Tree,
        Focus::Content => BindingScope::Content,
    };
    let candidates = match app.focus {
        Focus::Tree => TREE_HINTS,
        Focus::Content if app.current_file.is_some() || !app.content.is_empty() => CONTENT_HINTS,
        Focus::Content => EMPTY_CONTENT_HINTS,
    };
    let resolve = |&(action, label): &(&'static str, &'static str)| {
        if app.check_applicability(action).is_err() {
            return None;
        }
        let key = app.keys().binding_in(action, scope)?.key_label();
        Some(Hint { action, key, label })
    };
    let mut hints: Vec<Hint> = candidates
        .iter()
        .filter_map(resolve)
        .take(MAX_CONTEXT_HINTS)
        .collect();
    hints.extend(TAIL_HINTS.iter().filter_map(resolve));
    hints
}

/// Draws the hint row into `area` and returns each hint's click target.
pub(crate) fn draw_hints(f: &mut Frame, app: &App, area: Rect) -> Vec<(Rect, &'static str)> {
    let theme = &app.theme;
    let key_style = Style::default()
        .fg(theme.accent)
        .add_modifier(Modifier::BOLD);
    let label_style = Style::default().fg(theme.dim);
    let mut spans = vec![Span::raw(" ")];
    let mut targets = Vec::new();
    let mut x = area.x + 1;
    let right = area.x + area.width;
    for hint in hint_items(app) {
        let key_width = crate::text_width::str_width(&hint.key) as u16;
        let label_width = crate::text_width::str_width(hint.label) as u16;
        let width = key_width + 1 + label_width;
        if x + width > right {
            break;
        }
        targets.push((Rect::new(x, area.y, width, 1), hint.action));
        spans.push(Span::styled(hint.key, key_style));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(hint.label, label_style));
        spans.push(Span::raw("   "));
        x += width + 3;
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
    targets
}

#[cfg(test)]
#[path = "hints_test.rs"]
mod tests;
