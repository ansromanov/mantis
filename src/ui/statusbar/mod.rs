//! Status-bar rendering: the bottom row (or two) of the UI.
//!
//! `draw_statusbar` renders the bottom row or rows of the screen. When an
//! overlay is active it shows that overlay's key hints; otherwise it summarizes
//! the focused panel, file path, git state, position, and active modes. Colors
//! come from the active theme, with per-segment overrides from
//! `[statusbar.colors]`. It is a read-only projection of `App` and is drawn
//! last so it reflects the final per-frame state. `hit_test` maps clicks to
//! actionable segments, while `segment_at` returns visible text for the
//! status-bar context menu.
//!
//! On narrow terminals the bar elides low-priority segments so it never
//! overflows `area.width`. Plugin and status messages (`P_META`) are dropped
//! first, then fold stats, badges, and file info (`P_INFO`), then git info;
//! error indicators — including a `plugin_error` action (protocol 3+, styled
//! distinctly from routine `show_message` text) — and the version string are
//! always shown. When `[statusbar] height = 2`, the left-aligned segments fill
//! the top row and right-aligned segments the bottom row instead of sharing a
//! single row, so narrow terminals spread segments rather than dropping them;
//! elision still applies per row. Segments are joined by the configured
//! `separator` (default: a single space, reproducing the historical bytes
//! exactly). Keybinding hints are no longer rendered — the `?` help overlay
//! and the command palette are the discovery surfaces for bindings.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus};
use crate::git::{GitHead, GitRepoInfo};

pub(super) mod fit;
use self::fit::{fit_two_row, fit_two_sided, P_ERR, P_GIT, P_INFO, P_META, P_VER};

/// Named segment identifiers for status-bar alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusSegment {
    Badges,
    Worktrees,
    Scroll,
    Lnum,
    Type,
    JsonPath,
    FileInfo,
    Git,
    Errors,
    Folds,
    PluginFacts,
    Message,
    PluginError,
    Version,
    Update,
}

impl StatusSegment {
    fn id_str(self) -> &'static str {
        match self {
            StatusSegment::Badges => "badges",
            StatusSegment::Worktrees => "worktrees",
            StatusSegment::Scroll => "scroll",
            StatusSegment::Lnum => "lnum",
            StatusSegment::Type => "type",
            StatusSegment::JsonPath => "jsonpath",
            StatusSegment::FileInfo => "fileinfo",
            StatusSegment::Git => "git",
            StatusSegment::Errors => "errors",
            StatusSegment::Folds => "folds",
            StatusSegment::PluginFacts => "pluginfacts",
            StatusSegment::Message => "message",
            StatusSegment::PluginError => "pluginerror",
            StatusSegment::Version => "version",
            StatusSegment::Update => "update",
        }
    }

    /// Returns which side this segment belongs to in **default** mode
    /// (both `left` and `right` are `None`). Explicit mode uses `split_sides`
    /// directly and never calls this.
    fn side(self) -> StatusSide {
        if ["lnum", "type", "jsonpath", "git", "version", "update"].contains(&self.id_str()) {
            StatusSide::Right
        } else {
            StatusSide::Left
        }
    }

    /// Existing palette action id activated by clicking this segment.
    pub(crate) fn action_id(self) -> Option<&'static str> {
        match self {
            StatusSegment::Git => Some("compare_against"),
            StatusSegment::Worktrees => Some("worktree_picker"),
            StatusSegment::Lnum => Some("goto_line"),
            StatusSegment::Type => Some("theme_picker"),
            StatusSegment::Folds => Some("fold_all"),
            StatusSegment::Errors | StatusSegment::PluginError => Some("plugin_picker"),
            StatusSegment::Update => Some("show_about"),
            StatusSegment::FileInfo => Some("copy_path"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusSide {
    Left,
    Right,
}

pub(super) fn draw_statusbar(
    f: &mut Frame,
    app: &App,
    area: Rect,
) -> Vec<(StatusSegment, u16, u16)> {
    let theme = &app.theme;
    let base = if theme.is_monochrome() {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().bg(theme.selection_bg).fg(theme.text)
    };

    let (lines, segments) = if app.goto_line.is_some() {
        (vec![overlay_line(
            " type line number  Enter jump  Esc cancel  +N forward  -N back",
            base,
            area.width,
        )], Vec::new())
    } else if app.theme_picker.is_some() {
        (vec![overlay_line(
            " \u{2191}\u{2193} navigate  type to filter  Enter apply theme  Esc cancel",
            base,
            area.width,
        )], Vec::new())
    } else if app.history.is_some() {
        (vec![overlay_line(
            " \u{2191}\u{2193} navigate  type to filter  Enter show diff  Esc cancel",
            base,
            area.width,
        )], Vec::new())
    } else if app.search.is_some() {
        (vec![overlay_line(
            " \u{2191}\u{2193} navigate  Enter select  Tab toggle mode  Esc cancel",
            base,
            area.width,
        )], Vec::new())
    } else {
        (build_normal_lines(app, base, area.width), Vec::new())
    };

    f.render_widget(Paragraph::new(lines).style(base), area);
    segments
        .into_iter()
        .filter(|(segment, _, _)| segment.action_id().is_some())
        .map(|(segment, start, end)| {
            (
                segment,
                area.x.saturating_add(start),
                area.x.saturating_add(end),
            )
        })
        .collect()
}

/// Returns the segment under a click using geometry generated with the line.
pub(crate) fn hit_test(app: &App, column: u16, row: u16) -> Option<StatusSegment> {
    if row < app.statusbar_area.y || row >= app.statusbar_area.bottom()
        || column < app.statusbar_area.x || column >= app.statusbar_area.right()
    {
        return None;
    }
    app.statusbar_segments
        .iter()
        .find(|(_, start, end)| column >= *start && column < *end)
        .map(|(segment, _, _)| *segment)
}

/// Returns visible status-bar text under a right-click, excluding the spacer
/// between left and right groups and empty overlay hints.
pub(crate) fn segment_at(app: &App, area: Rect, column: u16, row: u16) -> Option<String> {
    if column < area.x || column >= area.right() || row < area.y || row >= area.bottom() {
        return None;
    }
    let lines = build_normal_lines(app, Style::default(), area.width);
    let row_index = usize::from(row.saturating_sub(area.y));
    let line = lines.get(row_index)?;
    let mut x = area.x;
    for span in &line.spans {
        let end = x.saturating_add(span.width() as u16);
        if column >= x && column < end {
            let text = span.content.to_string();
            return (!text.trim().is_empty()).then_some(text);
        }
        x = end;
    }
    None
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

/// Normal (non-overlay) status bar with priority-based elision and configurable
/// left/right alignment per segment. Returns one row in default `height = 1`
/// mode and two rows (left group on top, right group on the bottom) when the
/// config sets `height = 2`.
fn build_normal_lines(app: &App, base: Style, max_width: u16) -> Vec<Line<'static>> {
    let badge = base.fg(app.theme.accent).add_modifier(Modifier::BOLD);
    let err_style = base.fg(app.theme.diff_del).add_modifier(Modifier::BOLD);
    let dim = base.fg(app.theme.dim);

    let mut segs: Vec<(Span<'static>, StatusSegment, u8)> = Vec::new();

    // -- Priority 2: active-mode badges --
    if matches!(app.focus, Focus::Tree) && app.show_hidden {
        segs.push((
            Span::styled(" [hidden]", badge),
            StatusSegment::Badges,
            P_INFO,
        ));
    }
    if app.git_mode {
        let label = if app.git_mode_flat {
            " [git:flat]"
        } else {
            " [git]"
        };
        segs.push((Span::styled(label, badge), StatusSegment::Badges, P_INFO));
    }
    if app.worktree_count > 1 {
        segs.push((
            Span::styled(format!(" [worktrees: {}]", app.worktree_count), badge),
            StatusSegment::Worktrees,
            P_INFO,
        ));
    }
    if app.git_mode {
        if let Some(ref base) = app.compare_base {
            segs.push((
                Span::styled(format!(" [compare: {base}]"), badge),
                StatusSegment::Badges,
                P_INFO,
            ));
        }
        if let Some(ref commit) = app.commit_base {
            segs.push((
                Span::styled(format!(" [commit: {commit}]"), badge),
                StatusSegment::Badges,
                P_INFO,
            ));
        }
    }
    if app.show_blame && app.has_text_cursor() {
        segs.push((
            Span::styled(" [blame]", badge),
            StatusSegment::Badges,
            P_INFO,
        ));
    }
    let focus_label = match app.focus {
        Focus::Tree => " [focus: tree]",
        Focus::Content => " [focus: content]",
    };
    segs.push((
        Span::styled(focus_label, badge),
        StatusSegment::Badges,
        P_INFO,
    ));
    if app.auto_watch {
        segs.push((
            Span::styled(" [watch]", badge),
            StatusSegment::Badges,
            P_INFO,
        ));
    }
    if app.secret_masked {
        let label = if app.secret_revealed {
            " [secrets: revealed]"
        } else {
            " [secrets: masked]"
        };
        segs.push((Span::styled(label, badge), StatusSegment::Badges, P_INFO));
    }
    if !app.plugin_manager.is_empty() {
        segs.push((
            Span::styled(" [plugin]", badge),
            StatusSegment::Badges,
            P_INFO,
        ));
    }
    if app.is_diff && app.git_mode {
        let key = app.keys().label_for_action("toggle_diff_staged");
        if key.is_empty() {
            segs.push((
                Span::styled(format!(" [diff: {}]", app.diff_mode.label()), badge),
                StatusSegment::Badges,
                P_INFO,
            ));
        } else {
            segs.push((
                Span::styled(format!(" [diff: {} · {key}]", app.diff_mode.label()), badge),
                StatusSegment::Badges,
                P_INFO,
            ));
        }
    }

    // -- Priority 2: scroll percentage --
    if app.show_scroll_percentage && app.current_file.is_some() {
        let max = app.content_scroll_max();
        if max > 0 {
            let at_end = app.content_scroll >= max;
            let pct = (app.content_scroll * 100)
                .checked_div(max)
                .unwrap_or(0)
                .min(100);
            segs.push((
                Span::styled(
                    if at_end {
                        "  Bottom 100%".to_string()
                    } else {
                        format!("  Top {pct}%")
                    },
                    base,
                ),
                StatusSegment::Scroll,
                P_INFO,
            ));
        }
    }

    // -- Priority 2: active line number and language indicator --
    if app.current_file.is_some() && !app.is_diff {
        let ln = app.active_line + 1;
        segs.push((
            Span::styled(format!(" Cursor Ln {ln}"), dim),
            StatusSegment::Lnum,
            P_INFO,
        ));
        if let Some(ref syn) = app.current_syntax {
            segs.push((
                Span::styled(format!(" [{syn}]"), dim),
                StatusSegment::Type,
                P_INFO,
            ));
        }
        if app.is_json || app.is_jsonl {
            if let Some(Some(path)) = app.json_path_map.get(app.active_line) {
                if !path.is_empty() {
                    segs.push((
                        Span::styled(format!(" {path}"), dim),
                        StatusSegment::JsonPath,
                        P_INFO,
                    ));
                }
            }
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
            segs.push((Span::styled(label, style), StatusSegment::FileInfo, P_INFO));
        }
    }

    // -- Priority 3: git info --
    if let Some(ref info) = app.git_info {
        let fg = match info.head {
            GitHead::Detached => app.theme.git_conflict,
            GitHead::Rebase | GitHead::Merge => app.theme.git_progress,
            GitHead::Branch(_) if info.is_dirty() => app.theme.git_dirty,
            GitHead::Branch(_) => app.theme.git_clean,
        };
        segs.push((
            Span::styled(git_info_str(info), base.fg(fg)),
            StatusSegment::Git,
            P_GIT,
        ));
    }

    // -- Priority 4: error indicators --
    if app.walk_errors > 0 {
        segs.push((
            Span::styled(format!(" [!{}]", app.walk_errors), err_style),
            StatusSegment::Errors,
            P_ERR,
        ));
    }
    if app.config_error.is_some() {
        segs.push((
            Span::styled(" [config error]", err_style),
            StatusSegment::Errors,
            P_ERR,
        ));
    }
    if let Some(ref err) = app.yaml_error {
        let label = err.lines().next().unwrap_or(err);
        segs.push((
            Span::styled(format!(" [YAML: {label}]"), err_style),
            StatusSegment::Errors,
            P_ERR,
        ));
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
            StatusSegment::Folds,
            P_INFO,
        ));
    }

    // -- Priority 2: plugin-supplied status facts (`set_status_facts`) --
    if let Some(facts) = app
        .current_file
        .as_deref()
        .and_then(|p| app.plugin_status_facts.get(p))
    {
        segs.push((
            Span::styled(format!(" [{facts}]"), base.fg(app.theme.accent)),
            StatusSegment::PluginFacts,
            P_INFO,
        ));
    }

    // -- Priority 1: plugin / status messages --
    if let Some(ref msg) = app.plugin_message {
        segs.push((
            Span::styled(format!(" {msg}"), base.fg(app.theme.accent)),
            StatusSegment::Message,
            P_META,
        ));
    }
    if let Some(ref sm) = app.status_message {
        segs.push((
            Span::styled(format!(" {}", sm.text), base.fg(app.theme.accent)),
            StatusSegment::Message,
            P_META,
        ));
    }
    // A `plugin_error` action (protocol 3+) is distinct from routine
    // `show_message` text: it gets error styling and its own (higher)
    // priority so it survives eliding on narrow terminals alongside the
    // other error indicators above.
    if let Some(ref err) = app.plugin_error {
        segs.push((
            Span::styled(format!(" {err}"), err_style),
            StatusSegment::PluginError,
            P_ERR,
        ));
    }

    // -- Priority 4: update notice --
    if let Some(ref latest) = app.new_version_available {
        let update_style = base.fg(app.theme.accent_alt).add_modifier(Modifier::BOLD);
        segs.push((
            Span::styled(format!(" [update: {latest}]"), update_style),
            StatusSegment::Update,
            P_ERR,
        ));
    }

    // -- Priority 5: version (always kept) --
    segs.push((
        Span::styled(
            format!(" v{}", env!("CARGO_PKG_VERSION")),
            base.fg(app.theme.dim),
        ),
        StatusSegment::Version,
        P_VER,
    ));

    let statusbar = &app.config.statusbar;
    if statusbar.height >= 2 {
        let (top, bottom) = fit_two_row(segs, max_width as usize, statusbar, &app.theme);
        vec![top, bottom]
    } else {
        vec![fit_two_sided(
            segs,
            max_width as usize,
            statusbar,
            &app.theme,
        )]
    }
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
