//! The help popup.
//!
//! `draw_help` renders a centered, bordered overlay listing the application's
//! keybindings and actions grouped by area (navigation, content, search, git,
//! and so on). Key columns are built dynamically from the live `Keymap` so that
//! remapped keys are reflected immediately. Sections that cover hardcoded
//! overlay behaviours (in-file search, search/history popup, visual-line mode)
//! remain static reference text.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

struct HelpEntry {
    action_id: &'static str,
    desc: &'static str,
}

struct HelpSection {
    name: &'static str,
    entries: &'static [HelpEntry],
}

/// Keymap-driven help entries. Each entry's key label is queried from the live
/// `Keymap` so remapped bindings show correctly.
const KEYMAP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        name: "Global",
        entries: &[
            HelpEntry {
                action_id: "help",
                desc: "toggle this help",
            },
            HelpEntry {
                action_id: "switch_panel",
                desc: "switch panel",
            },
            HelpEntry {
                action_id: "quit",
                desc: "quit",
            },
            HelpEntry {
                action_id: "toggle_hidden",
                desc: "toggle hidden files",
            },
            HelpEntry {
                action_id: "theme_picker",
                desc: "pick a theme",
            },
            HelpEntry {
                action_id: "plugin_picker",
                desc: "plugin manager",
            },
            HelpEntry {
                action_id: "git_mode_toggle",
                desc: "toggle git mode (changed files only + diffs)",
            },
            HelpEntry {
                action_id: "open_in_editor",
                desc: "open file in $EDITOR",
            },
            HelpEntry {
                action_id: "copy_path",
                desc: "copy absolute path to clipboard",
            },
            HelpEntry {
                action_id: "copy_relative_path",
                desc: "copy path relative to tree root to clipboard",
            },
            HelpEntry {
                action_id: "recent_files",
                desc: "recent files picker",
            },
            HelpEntry {
                action_id: "git_mode_flat_toggle",
                desc: "toggle git flat/tree view (in git mode)",
            },
        ],
    },
    HelpSection {
        name: "Tree panel",
        entries: &[
            HelpEntry {
                action_id: "nav_up",
                desc: "move up",
            },
            HelpEntry {
                action_id: "nav_down",
                desc: "move down",
            },
            HelpEntry {
                action_id: "tree_expand",
                desc: "expand dir / open file",
            },
            HelpEntry {
                action_id: "tree_collapse",
                desc: "collapse dir",
            },
            HelpEntry {
                action_id: "tree_up_dir",
                desc: "go up one directory",
            },
            HelpEntry {
                action_id: "tree_collapse_all",
                desc: "collapse all directories",
            },
            HelpEntry {
                action_id: "tree_expand_all",
                desc: "expand all directories",
            },
            HelpEntry {
                action_id: "find_files",
                desc: "global fuzzy file-name picker",
            },
            HelpEntry {
                action_id: "search_files",
                desc: "tree filter / in-file search",
            },
            HelpEntry {
                action_id: "search_content",
                desc: "fuzzy content search",
            },
            HelpEntry {
                action_id: "reload",
                desc: "reload tree",
            },
        ],
    },
    HelpSection {
        name: "Content panel",
        entries: &[
            HelpEntry {
                action_id: "nav_up",
                desc: "scroll up",
            },
            HelpEntry {
                action_id: "nav_down",
                desc: "scroll down",
            },
            HelpEntry {
                action_id: "content_page_up",
                desc: "page scroll",
            },
            HelpEntry {
                action_id: "content_page_down",
                desc: "page scroll",
            },
            HelpEntry {
                action_id: "content_left",
                desc: "horizontal scroll",
            },
            HelpEntry {
                action_id: "content_right",
                desc: "horizontal scroll",
            },
            HelpEntry {
                action_id: "content_reset_col",
                desc: "reset horizontal scroll",
            },
            HelpEntry {
                action_id: "content_top",
                desc: "go to top",
            },
            HelpEntry {
                action_id: "content_bottom",
                desc: "go to bottom",
            },
            HelpEntry {
                action_id: "toggle_wrap",
                desc: "toggle word wrap",
            },
            HelpEntry {
                action_id: "toggle_line_numbers",
                desc: "toggle line numbers",
            },
            HelpEntry {
                action_id: "toggle_blame",
                desc: "toggle git blame gutter",
            },
            HelpEntry {
                action_id: "toggle_raw_markdown",
                desc: "toggle markdown render (md files)",
            },
            HelpEntry {
                action_id: "file_history",
                desc: "git history of current file",
            },
            HelpEntry {
                action_id: "toggle_diff_side_by_side",
                desc: "toggle side-by-side diff (in a diff)",
            },
            HelpEntry {
                action_id: "diff_hunk_next",
                desc: "next / previous hunk (in a diff)",
            },
            HelpEntry {
                action_id: "diff_hunk_prev",
                desc: "next / previous hunk (in a diff)",
            },
            HelpEntry {
                action_id: "fold_toggle",
                desc: "toggle fold at cursor",
            },
        ],
    },
];

/// Helper: compute the widest key label across all keymap-driven entries so
/// rows can be aligned with consistent padding. Capped at 14 to keep the
/// popup compact — unusually long labels are truncated with `…`.
fn max_key_width(app: &App) -> usize {
    let mut max_w = 0usize;
    for section in KEYMAP_SECTIONS {
        for entry in section.entries {
            let label = app.keys().labels_for_action(entry.action_id);
            let w = label.len();
            if w > max_w {
                max_w = w;
            }
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

pub(crate) fn draw_help(f: &mut Frame, app: &App, area: Rect) {
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

    let key_w = max_key_width(app);

    let mut rows: Vec<Line> = Vec::new();
    for section_def in KEYMAP_SECTIONS {
        rows.push(section(section_def.name));
        for entry in section_def.entries {
            let label = app.keys().labels_for_action(entry.action_id);
            let display = truncate_label(&label, key_w);
            let padded = format!("  {display:width$}  ", width = key_w);
            rows.push(Line::from(vec![key_style(padded), desc(entry.desc)]));
        }
        rows.push(gap.clone());
    }

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

    f.render_widget(Paragraph::new(rows), inner);
}
