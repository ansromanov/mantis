//! Status-bar rendering: the bottom line of the UI.
//!
//! `draw_statusbar` renders the single row at the bottom of the screen. Its
//! contents are context-sensitive: when an overlay is active it shows that
//! overlay's key hints, and otherwise it summarizes the current state - focused
//! panel, file path, git branch/HEAD info (`GitRepoInfo`/`GitHead`), position
//! and scroll percentage, and active mode flags. Colors come from the active
//! theme. It is a read-only projection of `App`; it never mutates state and is
//! drawn last so it always reflects the final per-frame state.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus};
use crate::git::{GitHead, GitRepoInfo};

pub(super) fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let base = Style::default().bg(theme.selection_bg).fg(theme.text);

    let spans: Vec<Span> = if app.theme_picker.is_some() {
        vec![Span::styled(
            " \u{2191}\u{2193} navigate  type to filter  Enter apply theme  Esc cancel",
            base,
        )]
    } else if app.history.is_some() {
        vec![Span::styled(
            " \u{2191}\u{2193} navigate  type to filter  Enter show diff  Esc cancel",
            base,
        )]
    } else if app.search.is_some() {
        vec![Span::styled(
            " \u{2191}\u{2193} navigate  Enter select  Tab toggle mode  Esc cancel",
            base,
        )]
    } else if app.visual_line.is_some() {
        let blame = if app.blame_panel {
            "  b hide blame"
        } else {
            "  b blame"
        };
        vec![Span::styled(
            format!(" VISUAL LINE  j/k extend  g/G top/bot{blame}  Esc exit"),
            base,
        )]
    } else {
        let mut spans = vec![];

        // Accent style for active-mode badges; bold to stand apart from the
        // dimmed hint text.
        let badge = base.fg(theme.accent).add_modifier(Modifier::BOLD);
        // Red+bold style for error indicators.
        let error = base.fg(theme.diff_del).add_modifier(Modifier::BOLD);
        // Hint text recedes behind the state indicators.
        let hint_style = base.fg(theme.dim);

        let hint = match app.focus {
            Focus::Tree => {
                " j/k nav  Enter/l expand  h collapse  / files  f content  t theme  Tab panel  q quit  ? help".to_string()
            }
            Focus::Content => {
                let md = if app.is_markdown {
                    if app.show_raw_markdown {
                        "  M render"
                    } else {
                        "  M raw"
                    }
                } else if app.is_json && !app.json_pretty_lines.is_empty() {
                    if app.show_pretty_json {
                        "  J raw"
                    } else {
                        "  J pretty"
                    }
                } else {
                    ""
                };
                let wrap = if app.word_wrap {
                    "  z no-wrap"
                } else {
                    "  z wrap"
                };
                let hscroll = if app.word_wrap {
                    ""
                } else {
                    "  \u{2190}/\u{2192} h-scroll  0 reset col"
                };
                format!(
                    " j/k scroll  PgUp/PgDn{}  g/G top/bot  H history  Tab panel  q quit{}{}",
                    hscroll, md, wrap
                )
            }
        };
        spans.push(Span::styled(hint, hint_style));

        // Active-mode badges (only meaningful in the tree panel).
        if matches!(app.focus, Focus::Tree) {
            if app.show_hidden {
                spans.push(Span::styled(" [hidden]", badge));
            }
            if app.git_mode {
                let label = if app.git_mode_flat {
                    " [git:flat]"
                } else {
                    " [git]"
                };
                spans.push(Span::styled(label, badge));
            }
        }

        if app.auto_watch {
            spans.push(Span::styled(" [watch]", badge));
        }

        if !app.plugin_manager.is_empty() {
            spans.push(Span::styled(" [plugin]", badge));
        }

        if app.show_scroll_percentage && app.current_file.is_some() {
            let max = app.content_scroll_max();
            if max > 0 {
                let pct = (app.content_scroll * 100)
                    .checked_div(max)
                    .unwrap_or(0)
                    .min(100);
                spans.push(Span::styled(format!("  {pct}%"), base));
            }
        }

        // File encoding and line-ending info (only when a real file is open)
        if app.show_file_info {
            if let Some(ref enc) = app.file_encoding {
                let (label, style) = if let Some(ref le) = app.file_line_ending {
                    let style = if le == "mixed" { error } else { base.fg(theme.dim) };
                    (format!(" [{enc} {le}]"), style)
                } else {
                    (format!(" [{enc}]"), base.fg(theme.dim))
                };
                spans.push(Span::styled(label, style));
            }
        }

        if let Some(ref info) = app.git_info {
            // Semantic git color: green clean, yellow dirty, red for a detached
            // HEAD, orange while a rebase/merge is in progress.
            let fg = match info.head {
                GitHead::Detached => theme.git_conflict,
                GitHead::Rebase | GitHead::Merge => theme.git_progress,
                GitHead::Branch(_) if info.is_dirty() => theme.git_dirty,
                GitHead::Branch(_) => theme.git_clean,
            };
            spans.push(Span::styled(git_info_str(info), base.fg(fg)));
        }

        if app.walk_errors > 0 {
            spans.push(Span::styled(format!(" [!{}]", app.walk_errors), error));
        }

        if app.config_error.is_some() {
            spans.push(Span::styled(" [config error]", error));
        }

        // YAML validation error indicator
        if let Some(ref err) = app.yaml_error {
            let label = err.lines().next().unwrap_or(err);
            spans.push(Span::styled(format!(" [YAML: {label}]"), error));
        }

        // YAML anchor/alias/fold stats (only when a YAML file is open)
        if !app.yaml_fold_regions.is_empty() {
            let folded_count = app.yaml_folded.len();
            let total_regions = app.yaml_fold_regions.len();
            let anchor_info = if app.yaml_anchor_count > 0 || app.yaml_alias_count > 0 {
                format!(" &{} *{}", app.yaml_anchor_count, app.yaml_alias_count)
            } else {
                String::new()
            };
            spans.push(Span::styled(
                format!(" [Y{anchor_info} {folded_count}/{total_regions}]"),
                base.fg(theme.accent),
            ));
        }

        if let Some(ref msg) = app.plugin_message {
            spans.push(Span::styled(format!(" {msg}"), base.fg(theme.accent)));
        }

        spans.push(Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            base.fg(theme.dim),
        ));

        spans
    };

    f.render_widget(Paragraph::new(Line::from(spans)).style(base), area);
}

fn git_info_str(info: &GitRepoInfo) -> String {
    let head = info.head.display();
    let mut parts = Vec::new();

    if info.ahead > 0 {
        parts.push(format!("\u{2191}{}", info.ahead));
    }
    if info.behind > 0 {
        parts.push(format!("\u{2193}{}", info.behind));
    }
    if info.is_dirty() {
        parts.push(format!("+{}", info.total_changed));
    }

    if parts.is_empty() {
        format!(" [{}]", head)
    } else {
        format!(" [{} {}]", head, parts.join(" "))
    }
}

#[cfg(test)]
#[path = "statusbar_test.rs"]
mod tests;
