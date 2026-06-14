use ratatui::{
    layout::Rect,
    style::Style,
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
    } else {
        let mut spans = vec![];

        let hint = match app.focus {
            Focus::Tree => {
                let hidden = if app.show_hidden { " [hidden]" } else { "" };
                let git = if app.git_mode {
                    if app.git_mode_flat {
                        " [git:flat]"
                    } else {
                        " [git]"
                    }
                } else {
                    ""
                };
                format!(
                    " j/k nav  Enter/l expand  h collapse  / files  f content  t theme  Tab panel  q quit  ? help{}{}",
                    hidden, git
                )
            }
            Focus::Content => {
                let md = if app.is_markdown {
                    if app.show_raw_markdown {
                        "  M render"
                    } else {
                        "  M raw"
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
        spans.push(Span::styled(hint, base));

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

        if let Some(ref info) = app.git_info {
            let dirty = info.is_dirty()
                || matches!(
                    info.head,
                    GitHead::Detached | GitHead::Rebase | GitHead::Merge
                );
            let fg = if dirty {
                theme.git_dirty
            } else {
                theme.git_clean
            };
            spans.push(Span::styled(git_info_str(info), base.fg(fg)));
        }

        if app.walk_errors > 0 {
            spans.push(Span::styled(
                format!(" [!{}]", app.walk_errors),
                base.fg(theme.diff_del),
            ));
        }

        if app.config_error.is_some() {
            spans.push(Span::styled(" [config error]", base.fg(theme.diff_del)));
        }

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
