//! The help popup.
//!
//! `draw_help` renders a centered, bordered overlay listing the application's
//! keybindings and actions grouped by area (navigation, content, search, git,
//! and so on). Key columns are built dynamically from the live `Keymap` so that
//! remapped keys are reflected immediately. Sections that cover hardcoded
//! overlay behaviours (in-file search, search/history popup, visual-line mode)
//! remain static reference text.
//!
//! The `Global`/`Tree panel`/`Content panel` sections are built by
//! `keymap_help_sections`, which groups `crate::actions::ACTIONS` by its
//! `.help` field, preserving the order actions first appear in `ACTIONS`.
//! This replaces the old hand-duplicated `KEYMAP_SECTIONS` table, so a new
//! action only needs a `help: Some((section, desc))` entry in `ACTIONS` to
//! show up here. `nav_up`/`nav_down` are the one exception: they are bound
//! both in the tree panel (move selection) and the content panel (scroll),
//! and since an `ActionSpec` has only one `help` slot, the Content panel's
//! second meaning is appended as two hand-written rows rather than a second
//! registry entry. The dedicated Git section stays partially hand-assembled
//! (it interleaves static orientation rows - tree colors, status bar legend -
//! with keymap-driven rows) but its `GIT_KEYMAP_ENTRIES` ids are all
//! canonical `ACTIONS` ids, checked by `actions_test.rs`.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::actions::ACTIONS;
use crate::app::App;

use super::util::centered_rect;

/// Groups `ACTIONS`' `.help` entries by section name, preserving both the
/// order sections first appear and the order of entries within each section.
fn keymap_help_sections() -> Vec<(&'static str, Vec<(&'static str, &'static str)>)> {
    let mut sections: Vec<(&'static str, Vec<(&'static str, &'static str)>)> = Vec::new();
    for action in ACTIONS {
        let Some((section_name, desc)) = action.help else {
            continue;
        };
        match sections.iter_mut().find(|(name, _)| *name == section_name) {
            Some((_, entries)) => entries.push((action.id, desc)),
            None => sections.push((section_name, vec![(action.id, desc)])),
        }
    }
    // nav_up/nav_down are also bound in the content panel (scrolling), a
    // second meaning `ActionSpec::help` has no room for; append it here.
    if let Some((_, entries)) = sections
        .iter_mut()
        .find(|(name, _)| *name == "Content panel")
    {
        entries.insert(0, ("nav_up", "scroll up"));
        entries.insert(1, ("nav_down", "scroll down"));
    }
    sections
}

/// Git-specific keybinding rows rendered in the dedicated Git section.
/// Each tuple is `(action_id, user-facing description)`; `action_id` must be
/// a canonical id from `crate::actions::ACTIONS` (checked by
/// `actions_test.rs`), even though the description here is git-specific
/// phrasing distinct from that action's `ACTIONS` entry (if it has one).
const GIT_KEYMAP_ENTRIES: &[(&str, &str)] = &[
    (
        "git_mode_toggle",
        "show only changed files; each file opens its diff",
    ),
    (
        "git_mode_flat_toggle",
        "toggle flat list / nested tree (git mode only)",
    ),
    (
        "toggle_diff_side_by_side",
        "toggle side-by-side / unified diff",
    ),
    (
        "toggle_diff_staged",
        "cycle diff source: all (vs HEAD) -> staged -> unstaged",
    ),
    ("diff_hunk_next", "jump to next change hunk"),
    ("diff_hunk_prev", "jump to previous change hunk"),
    (
        "blame_line",
        "blame current line: hash  author  when  summary",
    ),
    (
        "file_history",
        "pick a commit -> view its diff vs your working tree",
    ),
];

/// Helper: compute the widest key label across all keymap-driven entries so
/// rows can be aligned with consistent padding. Capped at 14 to keep the
/// popup compact — unusually long labels are truncated with `…`.
fn max_key_width(
    app: &App,
    sections: &[(&'static str, Vec<(&'static str, &'static str)>)],
) -> usize {
    let mut max_w = 0usize;
    for (_, entries) in sections {
        for &(action_id, _) in entries {
            let label = app.keys().labels_for_action(action_id);
            let w = label.len();
            if w > max_w {
                max_w = w;
            }
        }
    }
    for &(action_id, _) in GIT_KEYMAP_ENTRIES {
        let label = app.keys().labels_for_action(action_id);
        let w = label.len();
        if w > max_w {
            max_w = w;
        }
    }
    max_w.min(14)
}

/// Truncate a label to at most `max_len` chars, adding `…` when it exceeds.
fn truncate_label(label: &str, max_len: usize) -> String {
    if label.len() <= max_len {
        label.to_string()
    } else if max_len <= 1 {
        "…".to_string()
    } else {
        let mut s: String = label.chars().take(max_len - 1).collect();
        s.push('…');
        s
    }
}

pub(crate) fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(72, 80, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Help — ? / Esc to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let key_style = |k: String| {
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

    let keymap_sections = keymap_help_sections();
    let key_w = max_key_width(app, &keymap_sections);

    let mut rows: Vec<Line> = Vec::new();
    for (section_name, entries) in &keymap_sections {
        rows.push(section(section_name));
        for &(action_id, entry_desc) in entries {
            let label = app.keys().labels_for_action(action_id);
            let display = truncate_label(&label, key_w);
            let padded = format!("  {display:width$}  ", width = key_w);
            rows.push(Line::from(vec![key_style(padded), desc(entry_desc)]));
        }
        rows.push(gap.clone());
    }

    // Git section — static orientation rows followed by keymap-driven bindings.
    rows.push(section("Git"));
    let static_key_span = |label: &'static str| {
        Span::styled(
            format!("  {:<width$}  ", label, width = key_w),
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )
    };
    let indent_span = || {
        Span::styled(
            format!("  {:<width$}  ", "", width = key_w),
            Style::default().fg(theme.accent_alt),
        )
    };
    rows.push(Line::from(vec![
        static_key_span("Tree colors"),
        desc("green = new   yellow = modified   red = deleted   gray = ignored"),
    ]));
    rows.push(Line::from(vec![
        indent_span(),
        desc("a folder takes the color of changes inside it"),
    ]));
    rows.push(Line::from(vec![
        static_key_span("Status bar"),
        desc("[branch  +ahead -behind  N changed]"),
    ]));
    for &(action_id, description) in GIT_KEYMAP_ENTRIES {
        let label = app.keys().labels_for_action(action_id);
        let display = truncate_label(&label, key_w);
        let padded = format!("  {display:width$}  ", width = key_w);
        rows.push(Line::from(vec![key_style(padded), desc(description)]));
    }
    rows.push(gap.clone());

    // Non-keymap sections — hardcoded overlay behaviours, kept as static text.
    rows.push(section("Visual-line mode"));
    rows.push(Line::from(vec![
        Span::styled(
            "  j/k / ↑↓       ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("extend selection"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  g / G           ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("extend to top / bottom"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  b               ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("blame selected lines"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  Esc             ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("exit visual-line mode"),
    ]));
    rows.push(gap.clone());
    rows.push(section("In-file search"));
    rows.push(Line::from(vec![
        Span::styled(
            "  n / N           ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("next / previous match"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  Enter / Esc     ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("close search"),
    ]));
    rows.push(gap.clone());
    rows.push(section("Search / history popup"));
    rows.push(Line::from(vec![
        Span::styled(
            "  Tab             ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("switch files ↔ content mode"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  Enter           ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("open result / show diff"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  ↑↓              ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("navigate results"),
    ]));
    rows.push(Line::from(vec![
        Span::styled(
            "  Esc             ",
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        ),
        desc("close popup"),
    ]));

    let total_rows = rows.len();
    let visible = inner.height as usize;
    let max_scroll = total_rows.saturating_sub(visible);
    if app.help_scroll > max_scroll {
        app.help_scroll = max_scroll;
    }

    f.render_widget(
        Paragraph::new(rows).scroll((app.help_scroll as u16, 0)),
        inner,
    );

    if max_scroll > 0 {
        let indicator_y = if total_rows > 0 {
            (app.help_scroll as f64 * (inner.height as f64 - 2.0) / max_scroll as f64).round()
                as u16
        } else {
            0
        };
        let indicator_y = (indicator_y + inner.y).min(inner.bottom().saturating_sub(2));
        let indicator_chars = if app.help_scroll == 0 {
            " ▲ "
        } else if app.help_scroll >= max_scroll {
            " ▼ "
        } else {
            " ║ "
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                indicator_chars,
                Style::default().fg(theme.dim),
            ))),
            Rect {
                x: inner.right().saturating_sub(3),
                y: indicator_y,
                width: 3,
                height: 1,
            },
        );
    }
}
