//! The help popup.
//!
//! `draw_help` renders a centered, bordered overlay listing the application's
//! keybindings and actions grouped by tab (Getting started, Navigation, Content,
//! Search, Git, Settings, Themes, Plugins, Mouse). The user can switch tabs using
//! Left/Right arrows, h/l, or Tab/Shift-Tab, and scroll using Up/Down arrows or j/k.
//! Key columns are built dynamically from the live `Keymap` so that remapped
//! keys are reflected immediately.
//!
//! Each tab ends with a "Full docs" footer pointing at the `docs/src/*.md`
//! file that covers the topic in depth (see `HELP_DOC_LINKS`) — the sync
//! strategy between in-app help and the mdbook (#304).

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

pub const HELP_TABS: &[&str] = &[
    "Getting started",
    "Navigation",
    "Content",
    "Search",
    "Git",
    "Settings",
    "Themes",
    "Plugins",
    "Mouse",
];

/// `docs/src/*.md` file backing each `HELP_TABS` entry (same index order).
///
/// In-app help is a concise operational summary, not a copy of the mdbook —
/// duplicating full prose in both places would drift silently. Instead each
/// tab footer points at the doc file that covers it in depth, and
/// `help_test.rs::help_doc_links_point_to_existing_files` fails the build if
/// a link starts pointing at a file that was renamed or removed. That test is
/// the sync strategy: it doesn't catch content drift within a file, but it
/// guarantees the pointer from help to docs never dangles.
pub(super) const HELP_DOC_LINKS: &[&str] = &[
    "introduction.md",
    "usage.md",
    "usage.md",
    "usage.md",
    "git.md",
    "configuration.md",
    "themes.md",
    "plugins.md",
    "usage.md",
];

/// Git-specific keybinding rows rendered in the dedicated Git section.
/// Each tuple is `(action_id, user-facing description)`; `action_id` must be
/// a canonical id from `crate::actions::ACTIONS` (checked by
/// `help_test.rs::git_keymap_entries_ids_are_canonical_actions`).
#[allow(dead_code)]
pub(super) const GIT_KEYMAP_ENTRIES: &[(&str, &str)] = &[
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

/// Calculate the click ranges for each tab relative to the starting x coordinate.
pub(crate) fn help_tab_ranges(start_x: u16) -> Vec<(u16, u16)> {
    let mut current_x = start_x;
    let mut ranges = Vec::new();
    for (i, tab_name) in HELP_TABS.iter().enumerate() {
        if i > 0 {
            current_x += 3; // for " · "
        }
        let length = (tab_name.len() + 2) as u16; // for " {} "
        ranges.push((current_x, current_x + length));
        current_x += length;
    }
    ranges
}

/// Horizontal scroll offset for the tab bar so the active tab stays fully
/// visible within `available_width` columns. The full tab bar (all 9 tabs)
/// is wider than most terminal widths, so without scrolling, tabs past the
/// visible edge would be unreachable by mouse and invisible even though
/// keyboard cycling can still select them.
pub(crate) fn help_tab_scroll_offset(help_tab: usize, available_width: u16) -> u16 {
    let ranges = help_tab_ranges(0);
    let Some(&(_, total_width)) = ranges.last() else {
        return 0;
    };
    if total_width <= available_width {
        return 0;
    }
    let max_offset = total_width - available_width;
    let Some(&(start, end)) = ranges.get(help_tab) else {
        return 0;
    };
    let mut offset = if end > available_width {
        (end - available_width).min(max_offset)
    } else {
        0
    };
    if start < offset {
        offset = start;
    }
    offset
}

/// Lookup action description from ACTIONS registry. Falls back to the
/// palette label when the action has no dedicated `.help` description, so
/// palette-only actions (e.g. `toggle_watch`, `goto_line`) still render a
/// readable row instead of a blank description.
fn action_desc(action_id: &str) -> &'static str {
    let Some(action) = crate::actions::ACTIONS.iter().find(|a| a.id == action_id) else {
        return "";
    };
    action
        .help
        .map(|(_, desc)| desc)
        .or(action.palette)
        .unwrap_or("")
}

pub(crate) fn draw_help(f: &mut Frame, app: &mut App, area: Rect) {
    let theme = &app.theme;
    let popup = centered_rect(80, 80, area);
    app.help_area = popup;
    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Help — ? / Esc to close ")
        .borders(Borders::ALL)
        .style(Style::default().bg(theme.background))
        .border_style(Style::default().fg(theme.accent_alt));

    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // tab bar
            Constraint::Length(1), // separator rule
            Constraint::Min(0),    // scrollable content
        ])
        .split(inner);

    let tab_bar_area = chunks[0];
    let separator_area = chunks[1];
    let content_area = chunks[2];

    // 1. Draw Tab Bar
    let mut tab_spans = Vec::new();
    for (i, &tab_name) in HELP_TABS.iter().enumerate() {
        if i > 0 {
            tab_spans.push(Span::styled(" · ", Style::default().fg(theme.dim)));
        }
        if i == app.help_tab {
            tab_spans.push(Span::styled(
                format!(" {} ", tab_name),
                Style::default()
                    .fg(theme.accent)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
        } else {
            tab_spans.push(Span::styled(
                format!(" {} ", tab_name),
                Style::default().fg(theme.text).add_modifier(Modifier::DIM),
            ));
        }
    }
    let tab_scroll = help_tab_scroll_offset(app.help_tab, tab_bar_area.width);
    f.render_widget(
        Paragraph::new(Line::from(tab_spans)).scroll((0, tab_scroll)),
        tab_bar_area,
    );

    // 2. Draw Separator Line
    let h_rule = Line::from(vec![Span::styled(
        "─".repeat(separator_area.width as usize),
        Style::default().fg(theme.dim),
    )]);
    f.render_widget(Paragraph::new(h_rule), separator_area);

    // 3. Build Content Rows for Active Tab
    let key_w = 16;
    let key_style = |k: String| {
        Span::styled(
            k,
            Style::default()
                .fg(theme.accent_alt)
                .add_modifier(Modifier::BOLD),
        )
    };
    let desc = |d: &'static str| Span::styled(d, Style::default().fg(theme.text));
    let bold_text = |s: &'static str| {
        Line::from(vec![Span::styled(
            s,
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        )])
    };
    let normal_text =
        |s: &'static str| Line::from(vec![Span::styled(s, Style::default().fg(theme.text))]);
    let section = |s: &'static str| {
        Line::from(vec![Span::styled(
            s,
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )])
    };
    let gap = Line::from("");

    let row_key = |action_id: &str| {
        let label = app.keys().labels_for_action(action_id);
        let display = truncate_label(&label, key_w);
        let padded = format!("  {display:width$}  ", width = key_w);
        let entry_desc = action_desc(action_id);
        Line::from(vec![key_style(padded), desc(entry_desc)])
    };

    let row_key_custom = |action_id: &str, entry_desc: &'static str| {
        let label = app.keys().labels_for_action(action_id);
        let display = truncate_label(&label, key_w);
        let padded = format!("  {display:width$}  ", width = key_w);
        Line::from(vec![key_style(padded), desc(entry_desc)])
    };

    let row_static_key = |key_label: &'static str, entry_desc: &'static str| {
        let padded = format!("  {key_label:width$}  ", width = key_w);
        Line::from(vec![key_style(padded), desc(entry_desc)])
    };

    let mut rows: Vec<Line> = Vec::new();
    match app.help_tab {
        0 => {
            // Getting started
            rows.push(bold_text("Welcome to mantis!"));
            rows.push(gap.clone());
            rows.push(normal_text(
                "mantis is a terminal-based file tree viewer and content previewer.",
            ));
            rows.push(normal_text(
                "It features zero-config file browsing, syntax highlighting, fuzzy search,",
            ));
            rows.push(normal_text("and git integration."));
            rows.push(gap.clone());
            rows.push(section("Basic Concept"));
            rows.push(normal_text(
                "The layout is divided into two primary panels:",
            ));
            rows.push(normal_text(
                "  - Tree panel (left): Browse directory tree, expand/collapse folders.",
            ));
            rows.push(normal_text(
                "  - Content panel (right): View file contents, diffs, blame, and search.",
            ));
            rows.push(gap.clone());
            let cmd_palette_keys = app.keys().labels_for_action("command_palette");
            rows.push(Line::from(vec![Span::styled(
                format!(
                    "Actions labeled (palette only) are accessed via the command palette ({}).",
                    cmd_palette_keys
                ),
                Style::default().fg(theme.text),
            )]));
            rows.push(gap.clone());
            rows.push(section("Essential Keys"));
            rows.push(row_key("help"));
            rows.push(row_key("command_palette"));
            rows.push(row_key("switch_panel"));
            rows.push(row_key("quit"));
            rows.push(row_key("open_in_editor"));
            rows.push(row_key("recent_files"));
            rows.push(row_key("toggle_hidden"));
        }
        1 => {
            // Navigation
            rows.push(section("Panel Focus"));
            rows.push(row_key("switch_panel"));
            rows.push(gap.clone());
            rows.push(section("Tree Panel Navigation"));
            rows.push(row_key("nav_up"));
            rows.push(row_key("nav_down"));
            rows.push(row_key("tree_expand"));
            rows.push(row_key("tree_collapse"));
            rows.push(row_key("tree_up_dir"));
            rows.push(row_key("tree_collapse_all"));
            rows.push(row_key("tree_expand_all"));
            rows.push(row_key("reload"));
            rows.push(gap.clone());
            rows.push(section("Content Panel Navigation"));
            rows.push(row_key_custom("nav_up", "scroll up"));
            rows.push(row_key_custom("nav_down", "scroll down"));
            rows.push(row_key("content_page_up"));
            rows.push(row_key("content_page_down"));
            rows.push(row_key("content_top"));
            rows.push(row_key("content_bottom"));
            rows.push(row_key("content_left"));
            rows.push(row_key("content_right"));
            rows.push(row_key("content_reset_col"));
            rows.push(row_key("goto_line"));
            rows.push(gap.clone());
            rows.push(section("Code Folding"));
            rows.push(row_key("fold_toggle"));
            rows.push(row_static_key(
                "fold_all",
                "fold all regions in file (palette only)",
            ));
            rows.push(row_static_key(
                "unfold_all",
                "unfold all regions in file (palette only)",
            ));
        }
        2 => {
            // Content
            rows.push(section("Display Options"));
            rows.push(row_key("toggle_wrap"));
            rows.push(row_key("toggle_line_numbers"));
            rows.push(row_key("toggle_watch"));
            rows.push(row_static_key(
                "toggle_pretty_json",
                "toggle pretty-print for JSON files",
            ));
            rows.push(row_key("toggle_raw_markdown"));
            rows.push(gap.clone());
            rows.push(section("Clipboard Operations"));
            rows.push(row_key("copy_path"));
            rows.push(row_key("copy_relative_path"));
            rows.push(gap.clone());
            rows.push(section("File Previews"));
            rows.push(normal_text(
                "  - Binary files are automatically detected and previewed with size info.",
            ));
            rows.push(normal_text(
                "  - File encoding and line endings are probed on loading.",
            ));
        }
        3 => {
            // Search
            rows.push(section("Fuzzy Pickers"));
            rows.push(row_key("find_files"));
            rows.push(row_key("search_content"));
            rows.push(gap.clone());
            rows.push(section("Fuzzy Picker Controls"));
            rows.push(row_static_key(
                "Tab",
                "switch between results list and content preview",
            ));
            rows.push(row_static_key(
                "Enter",
                "open selected result / reveal matching line",
            ));
            rows.push(row_static_key(
                "Up / Down",
                "navigate through matching results list",
            ));
            rows.push(row_static_key("Esc", "close the fuzzy search popup"));
            rows.push(row_static_key(
                "Ctrl+A/W/R",
                "toggle case / whole-word / regex (content)",
            ));
            rows.push(gap.clone());
            rows.push(section("Filters & In-File Search"));
            rows.push(row_key("search_files"));
            rows.push(row_static_key(
                "/",
                "start incremental search inside current file",
            ));
            rows.push(row_static_key(
                "n / N",
                "jump to next / previous match (in-file search)",
            ));
            rows.push(row_static_key(
                "Ctrl+A/W/R",
                "toggle case / whole-word / regex",
            ));
            rows.push(row_static_key(
                "Enter / Esc",
                "close the in-file search bar",
            ));
        }
        4 => {
            // Git
            rows.push(section("Overview"));
            rows.push(normal_text(
                "mantis integrates with git to show repo changes:",
            ));
            rows.push(Line::from(vec![
                Span::styled("  Tree colors:  ", Style::default().fg(theme.accent_alt)),
                desc("green = new   yellow = modified   red = deleted   gray = ignored"),
            ]));
            rows.push(normal_text(
                "  A folder takes the color of changes inside it.",
            ));
            rows.push(Line::from(vec![
                Span::styled("  Status bar:   ", Style::default().fg(theme.accent_alt)),
                desc("[branch  +ahead -behind  N changed]"),
            ]));
            rows.push(gap.clone());
            rows.push(section("Git Modes"));
            rows.push(row_key_custom(
                "git_mode_toggle",
                "show only changed files; each file opens its diff",
            ));
            rows.push(row_key("git_mode_flat_toggle"));
            rows.push(gap.clone());
            rows.push(section("Diff Views & Navigation"));
            rows.push(row_key_custom(
                "toggle_diff_side_by_side",
                "toggle side-by-side / unified diff",
            ));
            rows.push(row_key_custom(
                "toggle_diff_staged",
                "cycle diff source: all (vs HEAD) -> staged -> unstaged",
            ));
            rows.push(row_key_custom("diff_hunk_next", "jump to next change hunk"));
            rows.push(row_key_custom(
                "diff_hunk_prev",
                "jump to previous change hunk",
            ));
            rows.push(gap.clone());
            rows.push(section("History & Blame"));
            rows.push(row_key("toggle_blame"));
            rows.push(row_key_custom(
                "blame_line",
                "blame current line: hash  author  when  summary",
            ));
            rows.push(row_key_custom(
                "file_history",
                "pick a commit -> view its diff vs your working tree",
            ));
        }
        5 => {
            // Settings
            rows.push(section("Configuration File"));
            rows.push(normal_text(
                "mantis settings are managed in a mantis.toml configuration file.",
            ));
            rows.push(normal_text(
                "You can open your configuration file directly in your system's editor:",
            ));
            rows.push(row_static_key(
                "open_config_in_editor",
                "open configuration file (palette only)",
            ));
            rows.push(gap.clone());
            rows.push(section("Configuration Path"));
            rows.push(normal_text("The config file is located at:"));
            rows.push(normal_text(
                "  - macOS/Linux: $XDG_CONFIG_HOME/mantis/mantis.toml (defaults to ~/.config/mantis/mantis.toml)",
            ));
            rows.push(normal_text(
                "  - Windows:     %APPDATA%\\mantis\\mantis.toml",
            ));
            rows.push(gap.clone());
            rows.push(section("Key Options"));
            rows.push(normal_text("  - [tree]: width, show_hidden"));
            rows.push(normal_text(
                "  - [content]: line_numbers, word_wrap, scrollbar, tab_width",
            ));
            rows.push(normal_text("  - [git]: ignore_gitignore"));
            rows.push(normal_text(
                "  - [keys]: custom keybindings mapping any action to keys",
            ));
        }
        6 => {
            // Themes
            rows.push(section("Theme Selection"));
            rows.push(row_key("theme_picker"));
            rows.push(gap.clone());
            rows.push(section("Built-in Presets"));
            rows.push(normal_text("  - default: Classic dark/gray layout"));
            rows.push(normal_text(
                "  - monokai: High contrast vintage code colors",
            ));
            rows.push(normal_text(
                "  - solarized: Balanced light/dark solarized palette",
            ));
            rows.push(normal_text(
                "  - catppuccin: Pastel color scheme (Catppuccin Macchiato)",
            ));
            rows.push(normal_text("  - synthwave84: Retro neon colors"));
            rows.push(gap.clone());
            rows.push(section("Custom Themes"));
            rows.push(normal_text(
                "You can define custom themes in the themes/ directory at:",
            ));
            rows.push(normal_text(
                "  - macOS/Linux: $XDG_CONFIG_HOME/mantis/themes/*.toml (defaults to ~/.config/mantis/themes/*.toml)",
            ));
            rows.push(normal_text(
                "  - Windows:     %APPDATA%\\mantis\\themes\\*.toml",
            ));
        }
        7 => {
            // Plugins
            rows.push(section("Overview"));
            rows.push(normal_text(
                "Plugins are external processes communicating via newline-delimited JSON",
            ));
            rows.push(normal_text(
                "events on stdin/stdout to extend mantis capabilities (syntax highlighting, language folding).",
            ));
            rows.push(gap.clone());
            rows.push(section("Plugin Manager"));
            rows.push(row_key("plugin_picker"));
            rows.push(gap.clone());
            rows.push(section("Plugin Location"));
            rows.push(normal_text("Plugins are discovered and loaded from:"));
            rows.push(normal_text(
                "  - macOS/Linux: $XDG_CONFIG_HOME/mantis/plugins/ (defaults to ~/.config/mantis/plugins/)",
            ));
            rows.push(normal_text("  - Windows:     %APPDATA%\\mantis\\plugins\\"));
            rows.push(gap.clone());
            rows.push(section("For Developers"));
            rows.push(normal_text(
                "Refer to the plugin development guidelines in the online mdbook",
            ));
            rows.push(normal_text(
                "or at docs/src/plugin-development.md in the repository.",
            ));
        }
        8 => {
            // Mouse
            rows.push(section("General Actions"));
            rows.push(row_static_key(
                "Left-Click",
                "Focus a panel / select a file or folder",
            ));
            rows.push(row_static_key(
                "Double-Click",
                "Set clicked folder as the new tree root (if on folder)",
            ));
            rows.push(row_static_key(
                "Scroll Wheel",
                "Scroll up / down in the hovered/focused panel",
            ));
            rows.push(gap.clone());
            rows.push(section("Breadcrumb Actions"));
            rows.push(row_static_key(
                "Single-Click",
                "Hover highlight path components",
            ));
            rows.push(row_static_key(
                "Double-Click",
                "Set clicked breadcrumb folder as the new tree root",
            ));
        }
        _ => {}
    }

    if let Some(&doc_file) = HELP_DOC_LINKS.get(app.help_tab) {
        rows.push(gap.clone());
        rows.push(Line::from(vec![
            Span::styled("Full docs: ", Style::default().fg(theme.dim)),
            Span::styled(
                format!("docs/src/{doc_file}"),
                Style::default()
                    .fg(theme.dim)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    let total_rows = rows.len();
    let visible = content_area.height as usize;
    let max_scroll = total_rows.saturating_sub(visible);
    if app.help_scroll > max_scroll {
        app.help_scroll = max_scroll;
    }

    f.render_widget(
        Paragraph::new(rows).scroll((app.help_scroll as u16, 0)),
        content_area,
    );

    if max_scroll > 0 {
        let indicator_y = if total_rows > 0 {
            (app.help_scroll as f64 * content_area.height.saturating_sub(2) as f64
                / max_scroll as f64)
                .round() as u16
        } else {
            0
        };
        let indicator_y = indicator_y
            .saturating_add(content_area.y)
            .min(content_area.bottom().saturating_sub(2));
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
                x: content_area.right().saturating_sub(3),
                y: indicator_y,
                width: 3,
                height: 1,
            },
        );
    }
}
