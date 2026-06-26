//! Status-bar rendering: the bottom line of the UI.
//!
//! `draw_statusbar` renders the single row at the bottom of the screen. Its
//! contents are context-sensitive: when an overlay is active it shows that
//! overlay's key hints, and otherwise it summarizes the current state - focused
//! panel, file path, git branch/HEAD info (`GitRepoInfo`/`GitHead`), position
//! and scroll percentage, and active mode flags. Colors come from the active
//! theme. It is a read-only projection of `App`; it never mutates state and is
//! drawn last so it always reflects the final per-frame state.
//!
//! On narrow terminals the bar elides low-priority segments so it never
//! overflows `area.width`. Keybinding hints are dropped first, then plugin
//! and status messages (`P_META`), then fold stats, badges, and file
//! info (`P_INFO`), then git info; error indicators and the version string
//! are always shown.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus};
use crate::git::{GitHead, GitRepoInfo};

/// Priority levels for status-bar segments (higher = kept when eliding).
const P_HINT: u8 = 0; // keybinding hints
const P_META: u8 = 1; // plugin/status messages
const P_INFO: u8 = 2; // fold stats, badges, scroll %, file encoding
const P_GIT: u8 = 3; // git branch info
const P_ERR: u8 = 4; // error indicators
const P_VER: u8 = 5; // version string

pub(super) fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let base = Style::default().bg(theme.selection_bg).fg(theme.text);

    let line = if app.goto_line.is_some() {
        overlay_line(
            " type line number  Enter jump  Esc cancel  +N forward  -N back",
            base,
            area.width,
        )
    } else if app.theme_picker.is_some() {
        overlay_line(
            " \u{2191}\u{2193} navigate  type to filter  Enter apply theme  Esc cancel",
            base,
            area.width,
        )
    } else if app.history.is_some() {
        overlay_line(
            " \u{2191}\u{2193} navigate  type to filter  Enter show diff  Esc cancel",
            base,
            area.width,
        )
    } else if app.search.is_some() {
        overlay_line(
            " \u{2191}\u{2193} navigate  Enter select  Tab toggle mode  Esc cancel",
            base,
            area.width,
        )
    } else {
        build_normal_line(app, base, area.width)
    };

    f.render_widget(Paragraph::new(line).style(base), area);
}

/// Overlay hint text, truncated with ellipsis if it exceeds `max_width`.
fn overlay_line(text: &str, style: Style, max_width: u16) -> Line<'static> {
    let full = Span::styled(text.to_string(), style);
    let w = full.width();
    if w as u16 <= max_width {
        return Line::from(vec![full]);
    }
    // Truncate with ellipsis: reserve 1 column for "…".
    let prefix_len = (max_width as usize).saturating_sub(1);
    let prefix: String = text.chars().take(prefix_len).collect();
    let display = if prefix.is_empty() {
        String::from("\u{2026}")
    } else {
        format!("{prefix}\u{2026}")
    };
    Line::from(vec![Span::styled(display, style)])
}

/// Normal (non-overlay) status bar with priority-based elision.
fn build_normal_line(app: &App, base: Style, max_width: u16) -> Line<'static> {
    let badge = base.fg(app.theme.accent).add_modifier(Modifier::BOLD);
    let err_style = base.fg(app.theme.diff_del).add_modifier(Modifier::BOLD);
    let dim = base.fg(app.theme.dim);

    let mut segs: Vec<(Span<'static>, u8)> = Vec::new();

    // -- Priority 0: keybinding hint (dropped first) --
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
            let diff_hint = if app.is_diff {
                "  S mode  D side-by-side  n/N hunks"
            } else {
                ""
            };
            format!(
                " j/k scroll  PgUp/PgDn{}  g/G top/bot  H history  Tab panel  q quit{}{}{}",
                hscroll, md, wrap, diff_hint
            )
        }
    };
    segs.push((Span::styled(hint, dim), P_HINT));

    // -- Priority 2: active-mode badges --
    if matches!(app.focus, Focus::Tree) {
        if app.show_hidden {
            segs.push((Span::styled(" [hidden]", badge), P_INFO));
        }
        if app.git_mode {
            let label = if app.git_mode_flat {
                " [git:flat]"
            } else {
                " [git]"
            };
            segs.push((Span::styled(label, badge), P_INFO));
        }
    }
    if app.auto_watch {
        segs.push((Span::styled(" [watch]", badge), P_INFO));
    }
    if !app.plugin_manager.is_empty() {
        segs.push((Span::styled(" [plugin]", badge), P_INFO));
    }

    // -- Priority 2: scroll percentage --
    if app.show_scroll_percentage && app.current_file.is_some() {
        let max = app.content_scroll_max();
        if max > 0 {
            let pct = (app.content_scroll * 100)
                .checked_div(max)
                .unwrap_or(0)
                .min(100);
            segs.push((Span::styled(format!("  {pct}%"), base), P_INFO));
        }
    }

    // -- Priority 2: active line number and language indicator --
    if app.current_file.is_some() && !app.is_diff {
        let ln = app.active_line + 1;
        segs.push((Span::styled(format!(" Ln {ln}"), dim), P_INFO));
        if let Some(ref syn) = app.current_syntax {
            segs.push((Span::styled(format!(" [{syn}]"), dim), P_INFO));
        }
    }

    // -- Priority 2: file encoding info --
    if app.show_file_info {
        if let Some(ref enc) = app.file_encoding {
            let (label, style) = if let Some(ref le) = app.file_line_ending {
                let s = if le == "mixed" { err_style } else { dim };
                (format!(" [{enc} {le}]"), s)
            } else {
                (format!(" [{enc}]"), dim)
            };
            segs.push((Span::styled(label, style), P_INFO));
        }
    }

    // -- Priority 3: git info --
    if let Some(ref plugin) = app.plugin_git_info {
        let fg = match plugin.state.as_str() {
            "conflict" => app.theme.git_conflict,
            "rebase" | "merge" => app.theme.git_progress,
            "dirty" => app.theme.git_dirty,
            _ => app.theme.git_clean,
        };
        let label = if plugin.dirty {
            format!(" [{} +1]", plugin.branch)
        } else {
            format!(" [{}]", plugin.branch)
        };
        segs.push((Span::styled(label, base.fg(fg)), P_GIT));
    } else if let Some(ref info) = app.git_info {
        let fg = match info.head {
            GitHead::Detached => app.theme.git_conflict,
            GitHead::Rebase | GitHead::Merge => app.theme.git_progress,
            GitHead::Branch(_) if info.is_dirty() => app.theme.git_dirty,
            GitHead::Branch(_) => app.theme.git_clean,
        };
        segs.push((Span::styled(git_info_str(info), base.fg(fg)), P_GIT));
    }

    // -- Priority 4: error indicators --
    if app.walk_errors > 0 {
        segs.push((
            Span::styled(format!(" [!{}]", app.walk_errors), err_style),
            P_ERR,
        ));
    }
    if app.config_error.is_some() {
        segs.push((Span::styled(" [config error]", err_style), P_ERR));
    }
    if let Some(ref err) = app.yaml_error {
        let label = err.lines().next().unwrap_or(err);
        segs.push((Span::styled(format!(" [YAML: {label}]"), err_style), P_ERR));
    }

    // -- Priority 2: fold stats --
    if !app.fold_regions.is_empty() {
        let folded_count = app.folded.len();
        let total_regions = app.fold_regions.len();
        let yaml_suffix = if app.yaml_anchor_count > 0 || app.yaml_alias_count > 0 {
            format!("&{} *{} ", app.yaml_anchor_count, app.yaml_alias_count)
        } else {
            String::new()
        };
        segs.push((
            Span::styled(
                format!(" [{yaml_suffix}{folded_count}/{total_regions}]"),
                base.fg(app.theme.accent),
            ),
            P_INFO,
        ));
    }

    // -- Priority 1: plugin / status messages --
    if let Some(ref msg) = app.plugin_message {
        segs.push((
            Span::styled(format!(" {msg}"), base.fg(app.theme.accent)),
            P_META,
        ));
    }
    if let Some(ref msg) = app.status_message {
        segs.push((
            Span::styled(format!(" {msg}"), base.fg(app.theme.accent)),
            P_META,
        ));
    }

    // -- Priority 5: version (always kept) --
    segs.push((
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            base.fg(app.theme.dim),
        ),
        P_VER,
    ));

    fit_segments(segs, max_width as usize)
}

/// From a list of `(Span, priority)` pairs, return a `Line` containing a
/// subset that fits within `max_width`.  Higher-priority items are kept
/// first; within the same priority level, items further to the right
/// (added later) are dropped before items on the left.
fn fit_segments(segs: Vec<(Span<'static>, u8)>, max_width: usize) -> Line<'static> {
    if segs.is_empty() || max_width == 0 {
        return Line::from(Vec::<Span>::new());
    }

    // Fast path: everything fits.
    let total: usize = segs.iter().map(|(s, _)| s.width()).sum();
    if total <= max_width {
        return Line::from(segs.into_iter().map(|(s, _)| s).collect::<Vec<_>>());
    }

    let n = segs.len();
    let mut keep = vec![true; n];

    // Indices sorted by priority (ascending) then position (descending),
    // so we remove lowest-priority, rightmost items first.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| segs[a].1.cmp(&segs[b].1).then(b.cmp(&a)));

    let mut current_width = total;

    for idx in order {
        if current_width <= max_width {
            break;
        }
        if keep[idx] {
            current_width -= segs[idx].0.width();
            keep[idx] = false;
        }
    }

    let final_spans: Vec<Span<'static>> = segs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, s)| s.0)
        .collect();

    Line::from(final_spans)
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
