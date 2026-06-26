//! The help popup.
//!
//! `draw_help` renders a centered, bordered overlay listing the application's
//! keybindings and actions grouped by area (navigation, content, search, git,
//! and so on). It is a static cheat sheet drawn from the current theme and is
//! toggled by the help key; the caller decides visibility from `App::show_help`.
//! The content is presentational only - it does not read the live `Keymap`, so
//! when keybindings or features change this listing must be updated by hand to
//! stay accurate. It serves as the primary in-app reference alongside the README.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

use super::util::centered_rect;

pub(crate) fn draw_help(f: &mut Frame, app: &App, area: Rect) {
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
        Line::from(vec![key("  .          "), desc("toggle hidden files")]),
        Line::from(vec![key("  t          "), desc("pick a theme")]),
        Line::from(vec![key("  p          "), desc("plugin manager")]),
        Line::from(vec![
            key("  Ctrl+G     "),
            desc("toggle git mode (changed files only + diffs)"),
        ]),
        Line::from(vec![key("  e          "), desc("open file in $EDITOR")]),
        Line::from(vec![
            key("  y          "),
            desc("copy absolute path to clipboard"),
        ]),
        Line::from(vec![
            key("  Y          "),
            desc("copy path relative to tree root to clipboard"),
        ]),
        Line::from(vec![key("  Ctrl+O     "), desc("recent files picker")]),
        Line::from(vec![
            key("  F          "),
            desc("toggle git flat/tree view (in git mode)"),
        ]),
        gap.clone(),
        section("Tree panel"),
        Line::from(vec![key("  j/k / ↑↓   "), desc("move up / down")]),
        Line::from(vec![key("  Enter/l/→  "), desc("expand dir / open file")]),
        Line::from(vec![key("  h/←        "), desc("collapse dir")]),
        Line::from(vec![key("  Backspace  "), desc("go up one directory")]),
        Line::from(vec![key("  -          "), desc("collapse all directories")]),
        Line::from(vec![key("  =          "), desc("expand all directories")]),
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
        Line::from(vec![key("  L          "), desc("toggle line numbers")]),
        Line::from(vec![key("  b          "), desc("toggle git blame gutter")]),
        Line::from(vec![
            key("  V          "),
            desc("visual-line mode (select lines)"),
        ]),
        Line::from(vec![
            key("  M          "),
            desc("toggle markdown render (md files)"),
        ]),
        Line::from(vec![
            key("  H          "),
            desc("git history of current file"),
        ]),
        Line::from(vec![
            key("  D          "),
            desc("toggle side-by-side diff (in a diff)"),
        ]),
        Line::from(vec![
            key("  n / N      "),
            desc("next / previous hunk (in a diff)"),
        ]),
        Line::from(vec![
            key("  /          "),
            desc("find in file (in-file search)"),
        ]),
        Line::from(vec![key("  Space      "), desc("toggle fold at cursor")]),
        gap.clone(),
        section("Visual-line mode"),
        Line::from(vec![key("  j/k / ↑↓   "), desc("extend selection")]),
        Line::from(vec![key("  g / G      "), desc("extend to top / bottom")]),
        Line::from(vec![key("  b          "), desc("blame selected lines")]),
        Line::from(vec![key("  Esc        "), desc("exit visual-line mode")]),
        gap.clone(),
        section("In-file search"),
        Line::from(vec![key("  n / N      "), desc("next / previous match")]),
        Line::from(vec![key("  Enter/Esc  "), desc("close search")]),
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
