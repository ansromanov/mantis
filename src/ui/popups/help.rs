//! The help popup.
//!
//! `draw_help` renders a centered overlay with action groups derived from the
//! canonical registry, plus prose-only topics for getting started, settings,
//! themes, plugins, and mouse gestures. The user can switch tabs using
//! Left/Right arrows, h/l, or Tab/Shift-Tab, and scroll using Up/Down arrows or j/k.
//! Key columns are built dynamically from the live `Keymap` so that remapped
//! keys are reflected immediately.
//!
//! Each tab ends with a "Full docs" footer pointing at the `docs/src/*.md`
//! file that covers the topic in depth; `help_doc_link` keeps those pointers
//! aligned with the mdbook.

use std::sync::LazyLock;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use super::util::centered_rect;
use crate::app::App;

/// Help tabs derived from the canonical action taxonomy plus prose-only topics.
static HELP_TABS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut tabs = vec!["Getting started"];
    tabs.extend(crate::actions::ACTION_CATEGORIES.iter().copied());
    tabs.extend(["Settings", "Themes", "Plugins", "Mouse"]);
    tabs
});

pub(crate) fn help_tabs() -> &'static [&'static str] {
    HELP_TABS.as_slice()
}

pub(crate) fn help_doc_link(tab: &str) -> &'static str {
    match tab {
        "Getting started" => "introduction.md",
        "Git" => "git.md",
        "Settings" => "configuration.md",
        "Themes" => "themes.md",
        "Plugins" => "plugins.md",
        _ => "usage.md",
    }
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

/// Calculate the click ranges for each tab relative to the starting x coordinate.
pub(crate) fn help_tab_ranges(start_x: u16) -> Vec<(u16, u16)> {
    let mut current_x = start_x;
    let mut ranges = Vec::new();
    for (i, tab_name) in help_tabs().iter().enumerate() {
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
/// visible within `available_width` columns. The full tab bar
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

/// Looks up an action description, falling back to its palette label when no
/// dedicated help text exists.
fn action_desc(action_id: &str) -> &'static str {
    let Some(action) = crate::actions::ACTIONS.iter().find(|a| a.id == action_id) else {
        return "";
    };
    action.help.or(action.palette).unwrap_or("")
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
    let tabs = help_tabs();
    for (i, &tab_name) in tabs.iter().enumerate() {
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
        let label = if action_id == "select_tab" {
            "Ctrl+1-9/0".to_string()
        } else {
            app.keys().labels_for_action(action_id)
        };
        let display = truncate_label(&label, key_w);
        let padded = format!("  {display:width$}  ", width = key_w);
        let entry_desc = action_desc(action_id);
        Line::from(vec![key_style(padded), desc(entry_desc)])
    };

    let row_static_key = |key_label: &'static str, entry_desc: &'static str| {
        let padded = format!("  {key_label:width$}  ", width = key_w);
        Line::from(vec![key_style(padded), desc(entry_desc)])
    };

    let mut rows: Vec<Line> = Vec::new();
    let tab = tabs.get(app.help_tab).copied().unwrap_or("Getting started");
    if tab == "Getting started" {
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
        rows.push(normal_text(
            "Browse the action categories for current keybindings and action descriptions.",
        ));
    } else if crate::actions::ACTION_CATEGORIES.contains(&tab) {
        if tab == "Git" {
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
        }
        rows.push(section(tab));
        for action in crate::actions::ACTIONS
            .iter()
            .filter(|action| action.category == tab && action.help.is_some())
        {
            rows.push(row_key(action.id));
        }
        if tab == "Navigate" {
            rows.push(gap.clone());
            rows.push(section("Context-sensitive Keys"));
            rows.push(row_static_key(
                "/",
                "context-sensitive: tree filter here; file search elsewhere",
            ));
            rows.push(gap.clone());
            rows.push(section("Picker Controls"));
            rows.push(row_static_key(
                "Tab",
                "switch between results and content preview",
            ));
            rows.push(row_static_key("Enter", "open the selected result"));
            rows.push(row_static_key("Up / Down", "move through matching results"));
            rows.push(row_static_key("Esc", "close the search popup"));
            rows.push(row_static_key(
                "Ctrl+A/W/R",
                "toggle case / whole-word / regex",
            ));
            rows.push(row_static_key(
                "/",
                "start incremental search inside the current file",
            ));
            rows.push(row_static_key(
                "n / N",
                "jump to the next / previous in-file match",
            ));
        }
    } else if tab == "Settings" {
        rows.push(section("Configuration"));
        rows.push(normal_text(
            "mantis settings are managed in a mantis.toml configuration file.",
        ));
        rows.push(row_static_key(
            "Palette",
            "open configuration file (palette only)",
        ));
        rows.push(gap.clone());
        rows.push(section("Configuration Path"));
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
    } else if tab == "Themes" {
        rows.push(section("Theme Selection"));
        rows.push(row_key("theme_picker"));
        rows.push(gap.clone());
        rows.push(section("Built-in Presets"));
        rows.push(normal_text(
            "  - default, monokai, solarized, catppuccin, synthwave84",
        ));
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
    } else if tab == "Plugins" {
        rows.push(section("Overview"));
        rows.push(normal_text(
            "Plugins are external processes that extend mantis capabilities.",
        ));
        rows.push(gap.clone());
        rows.push(section("Plugin Manager"));
        rows.push(row_key("plugin_picker"));
        rows.push(gap.clone());
        rows.push(section("For Developers"));
        rows.push(normal_text("Plugins are discovered and loaded from:"));
        rows.push(normal_text(
            "  - macOS/Linux: $XDG_CONFIG_HOME/mantis/plugins/ (defaults to ~/.config/mantis/plugins/)",
        ));
        rows.push(normal_text("  - Windows:     %APPDATA%\\mantis\\plugins\\"));
        rows.push(gap.clone());
        rows.push(normal_text(
            "See docs/src/plugin-development.md for the plugin protocol.",
        ));
    } else if tab == "Mouse" {
        rows.push(section("General Actions"));
        rows.push(row_static_key(
            "Left-Click",
            "focus a panel / select a file or folder",
        ));
        rows.push(row_static_key(
            "Double-Click",
            "set clicked folder as the new tree root",
        ));
        rows.push(row_static_key(
            "Scroll Wheel",
            "scroll the hovered/focused panel",
        ));
        rows.push(row_static_key("Right-Click", "open the context menu"));
        rows.push(row_static_key(
            "Mouse (status bar)",
            "Click a segment to open its picker or run its action",
        ));
        rows.push(gap.clone());
        rows.push(section("Breadcrumb Actions"));
        rows.push(row_static_key(
            "Single-Click",
            "hover highlight path components",
        ));
        rows.push(row_static_key(
            "Double-Click",
            "set clicked breadcrumb folder as the new tree root",
        ));
    }

    if !tab.is_empty() {
        let doc_file = help_doc_link(tab);
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
    app.help_scroll.clamp(max_scroll);

    f.render_widget(
        Paragraph::new(rows).scroll((app.help_scroll.scroll as u16, 0)),
        content_area,
    );

    if max_scroll > 0 {
        let indicator_y = if total_rows > 0 {
            (app.help_scroll.scroll as f64 * content_area.height.saturating_sub(2) as f64
                / max_scroll as f64)
                .round() as u16
        } else {
            0
        };
        let indicator_y = indicator_y
            .saturating_add(content_area.y)
            .min(content_area.bottom().saturating_sub(2));
        let indicator_chars = if app.help_scroll.scroll == 0 {
            " ▲ "
        } else if app.help_scroll.scroll >= max_scroll {
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
