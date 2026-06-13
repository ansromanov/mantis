use ratatui::{layout::Rect, style::Style, widgets::Paragraph, Frame};

use crate::app::{App, Focus};

pub(super) fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let hidden_indicator = if app.show_hidden { " [hidden]" } else { "" };
    let md_hint = if app.is_markdown {
        if app.show_raw_markdown {
            "  M render"
        } else {
            "  M raw"
        }
    } else {
        ""
    };
    let wrap_hint = if app.word_wrap {
        "  z no-wrap"
    } else {
        "  z wrap"
    };
    let hscroll_hint = if app.word_wrap {
        ""
    } else {
        "  ←/→ h-scroll  0 reset col"
    };
    let content_hint = format!(
        " j/k scroll  PgUp/PgDn{}  g/G top/bot  H history  Tab panel  q quit{}{}",
        hscroll_hint, md_hint, wrap_hint
    );
    let git_indicator = if app.git_mode {
        if app.git_mode_flat {
            " [git:flat]"
        } else {
            " [git]"
        }
    } else {
        ""
    };
    let tree_hint = format!(
        " j/k nav  Enter/l expand  h collapse  / files  f content  t theme  Tab panel  q quit  ? help{}{}",
        hidden_indicator, git_indicator
    );
    let git_info = app
        .git_branch
        .as_ref()
        .map(|b| {
            let dirty = app
                .git_status_map
                .values()
                .any(|&s| s != crate::git::GitStatus::Ignored);
            if dirty {
                format!(" [{} +]", b)
            } else {
                format!(" [{}]", b)
            }
        })
        .unwrap_or_default();

    let walk_err_indicator = if app.walk_errors > 0 {
        format!(" [!{}]", app.walk_errors)
    } else {
        String::new()
    };

    let config_err_indicator = if app.config_error.is_some() {
        " [config error]"
    } else {
        ""
    };

    let text: String = if app.theme_picker.is_some() {
        " ↑↓ navigate  type to filter  Enter apply theme  Esc cancel".into()
    } else if app.history.is_some() {
        " ↑↓ navigate  type to filter  Enter show diff  Esc cancel".into()
    } else if app.search.is_some() {
        " ↑↓ navigate  Enter select  Tab toggle mode  Esc cancel".into()
    } else {
        let base = match app.focus {
            Focus::Tree => tree_hint,
            Focus::Content => content_hint,
        };
        let scroll_max = app.content_scroll_max();
        if app.show_scroll_percentage && app.current_file.is_some() && scroll_max > 0 {
            let pct = (app.content_scroll * 100 / scroll_max).min(100);
            format!("{base}  {pct}%{git_info}{walk_err_indicator}{config_err_indicator}")
        } else {
            format!("{base}{git_info}{walk_err_indicator}{config_err_indicator}")
        }
    };

    f.render_widget(
        Paragraph::new(text).style(
            Style::default()
                .bg(app.theme.selection_bg)
                .fg(app.theme.text),
        ),
        area,
    );
}
