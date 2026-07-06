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
//! overflows `area.width`. Plugin and status messages (`P_META`) are dropped
//! first, then fold stats, badges, and file info (`P_INFO`), then git info;
//! error indicators — including a `plugin_error` action (protocol 3+, styled
//! distinctly from routine `show_message` text) — and the version string are
//! always shown. Keybinding hints are no longer rendered — the `?` help
//! overlay and the command palette are the discovery surfaces for bindings.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::{App, Focus};
use crate::config::StatusBarConfig;
use crate::git::{GitHead, GitRepoInfo};

/// Named segment identifiers for status-bar alignment.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusSegment {
    Badges,
    Scroll,
    Lnum,
    Type,
    FileInfo,
    Git,
    Errors,
    Folds,
    Message,
    PluginError,
    Version,
    Update,
}

impl StatusSegment {
    fn id_str(self) -> &'static str {
        match self {
            StatusSegment::Badges => "badges",
            StatusSegment::Scroll => "scroll",
            StatusSegment::Lnum => "lnum",
            StatusSegment::Type => "type",
            StatusSegment::FileInfo => "fileinfo",
            StatusSegment::Git => "git",
            StatusSegment::Errors => "errors",
            StatusSegment::Folds => "folds",
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
        if ["lnum", "type", "git", "version", "update"].contains(&self.id_str()) {
            StatusSide::Right
        } else {
            StatusSide::Left
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StatusSide {
    Left,
    Right,
}

/// Priority levels for status-bar segments (higher = kept when eliding).
const P_META: u8 = 1; // plugin/status messages
const P_INFO: u8 = 2; // fold stats, badges, scroll %, file encoding
const P_GIT: u8 = 3; // git branch info
const P_ERR: u8 = 4; // error indicators
const P_VER: u8 = 5; // version string

pub(super) fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let theme = &app.theme;
    let base = if theme.is_monochrome() {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default().bg(theme.selection_bg).fg(theme.text)
    };

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

/// Normal (non-overlay) status bar with priority-based elision and configurable
/// left/right alignment per segment.
fn build_normal_line(app: &App, base: Style, max_width: u16) -> Line<'static> {
    let badge = base.fg(app.theme.accent).add_modifier(Modifier::BOLD);
    let err_style = base.fg(app.theme.diff_del).add_modifier(Modifier::BOLD);
    let dim = base.fg(app.theme.dim);

    let mut segs: Vec<(Span<'static>, StatusSegment, u8)> = Vec::new();

    // -- Priority 2: active-mode badges --
    if matches!(app.focus, Focus::Tree) {
        if app.show_hidden {
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
    }
    if app.auto_watch {
        segs.push((
            Span::styled(" [watch]", badge),
            StatusSegment::Badges,
            P_INFO,
        ));
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
            let pct = (app.content_scroll * 100)
                .checked_div(max)
                .unwrap_or(0)
                .min(100);
            segs.push((
                Span::styled(format!("  {pct}%"), base),
                StatusSegment::Scroll,
                P_INFO,
            ));
        }
    }

    // -- Priority 2: active line number and language indicator --
    if app.current_file.is_some() && !app.is_diff {
        let ln = app.active_line + 1;
        segs.push((
            Span::styled(format!(" Ln {ln}"), dim),
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

    fit_two_sided(segs, max_width as usize, &app.config.statusbar)
}

/// From a list of `(Span, StatusSegment, priority)` pairs, return a `Line`
/// with segments split into left-aligned and right-aligned groups per config.
/// Higher-priority items are kept first; within the same priority level,
/// rightmost items are dropped first — across both groups.  The right group
/// is right-anchored as a block, with padding spaces in between.
///
/// In explicit allowlist mode (either `left` or `right` is `Some`), segments
/// not listed in either list are filtered out before any width/elision math.
fn fit_two_sided(
    segs: Vec<(Span<'static>, StatusSegment, u8)>,
    max_width: usize,
    config: &StatusBarConfig,
) -> Line<'static> {
    if segs.is_empty() || max_width == 0 {
        return Line::from(Vec::<Span>::new());
    }

    // Explicit allowlist mode: drop unlisted segments before width calc.
    let segs = if config.left.is_some() || config.right.is_some() {
        let left_ids = config.left.as_deref().unwrap_or(&[]);
        let right_ids = config.right.as_deref().unwrap_or(&[]);
        segs.into_iter()
            .filter(|(_, id, _)| {
                let name = id.id_str();
                left_ids.iter().any(|s| s == name) || right_ids.iter().any(|s| s == name)
            })
            .collect()
    } else {
        segs
    };

    // After filtering, might be empty.
    if segs.is_empty() {
        return Line::from(Vec::<Span>::new());
    }

    // Fast path: everything fits.
    let total: usize = segs.iter().map(|(s, _, _)| s.width()).sum();
    if total <= max_width {
        let (left, right) = split_sides(segs, config);
        return compose_left_right(left, right, max_width);
    }

    let n = segs.len();
    let mut keep = vec![true; n];

    // Indices sorted by priority (ascending) then position (descending),
    // so we remove lowest-priority, rightmost items first.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| segs[a].2.cmp(&segs[b].2).then(b.cmp(&a)));

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

    let kept: Vec<_> = segs
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, t)| t)
        .collect();

    let (left, right) = split_sides(kept, config);
    compose_left_right(left, right, max_width)
}

/// Split kept segments into left and right groups by config.
///
/// In explicit allowlist mode (either `left` or `right` is `Some`), the
/// groups are built by iterating each list in config order, preserving the
/// user-specified sequence. In default mode (both `None`), the order is the
/// build order, partitioned by `StatusSegment::side()`.
fn split_sides(
    segs: Vec<(Span<'static>, StatusSegment, u8)>,
    config: &StatusBarConfig,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let explicit = config.left.is_some() || config.right.is_some();
    if explicit {
        let left_ids = config.left.as_deref().unwrap_or(&[]);
        let right_ids = config.right.as_deref().unwrap_or(&[]);

        let mut left = Vec::new();
        let mut right = Vec::new();

        // Left side: iterate config order, pull matching built segment.
        for id in left_ids {
            if let Some((span, _, _)) = segs.iter().find(|(_, sid, _)| sid.id_str() == id.as_str())
            {
                left.push(span.clone());
            }
        }
        // Right side: iterate config order, pull matching built segment.
        for id in right_ids {
            if let Some((span, _, _)) = segs.iter().find(|(_, sid, _)| sid.id_str() == id.as_str())
            {
                right.push(span.clone());
            }
        }
        (left, right)
    } else {
        let mut left = Vec::new();
        let mut right = Vec::new();
        for (span, id, _) in segs {
            if id.side() == StatusSide::Right {
                right.push(span);
            } else {
                left.push(span);
            }
        }
        (left, right)
    }
}

/// Compose left and right span groups into a single Line, with a padding
/// gap between them.  Right group is right-anchored flush to max_width.
fn compose_left_right(
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    max_width: usize,
) -> Line<'static> {
    let left_w: usize = left.iter().map(Span::width).sum();
    let right_w: usize = right.iter().map(Span::width).sum();
    let gap = max_width.saturating_sub(left_w + right_w);

    let mut all: Vec<Span<'static>> = Vec::with_capacity(left.len() + 1 + right.len());
    all.extend(left);
    if gap > 0 {
        all.push(Span::raw(" ".repeat(gap)));
    }
    all.extend(right);
    Line::from(all)
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
