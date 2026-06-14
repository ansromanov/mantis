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
mod tests {
    use super::*;
    use crate::app::{App, Focus};
    use crate::config::Config;
    use crate::git::{GitHead, GitRepoInfo};
    use crate::search::{HistoryState, SearchState, ThemePicker};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::path::{Path, PathBuf};

    fn make_app() -> App {
        let cfg = Config {
            git_status: false,
            ..Config::default()
        };
        App::new(PathBuf::from("."), cfg, None, None).unwrap()
    }

    fn render_bar_width(app: &App, width: u16) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw_statusbar(f, app, f.area())).unwrap();
        let buf = terminal.backend().buffer();
        (0..width)
            .map(|x| buf[(x, 0)].symbol().to_string())
            .collect()
    }

    fn render_bar(app: &App) -> String {
        render_bar_width(app, 80)
    }

    #[test]
    fn hint_theme_picker() {
        let mut app = make_app();
        app.theme_picker = Some(ThemePicker::default());
        let text = render_bar(&app);
        assert!(text.contains("Enter apply theme"));
    }

    #[test]
    fn hint_history() {
        let mut app = make_app();
        app.history = Some(HistoryState::new(PathBuf::from("x"), vec![]));
        let text = render_bar(&app);
        assert!(text.contains("Enter show diff"));
    }

    #[test]
    fn hint_search() {
        let mut app = make_app();
        app.search = Some(SearchState::new(Path::new("."), false, true, 0));
        let text = render_bar(&app);
        assert!(text.contains("Enter select"));
    }

    #[test]
    fn tree_focus_default() {
        let app = make_app();
        let text = render_bar(&app);
        assert!(text.contains("j/k nav"));
        assert!(text.contains("v"));
    }

    #[test]
    fn tree_focus_hidden() {
        let mut app = make_app();
        app.show_hidden = true;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("[hidden]"));
    }

    #[test]
    fn tree_focus_git_mode() {
        let mut app = make_app();
        app.git_mode = true;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("[git]"));
    }

    #[test]
    fn tree_focus_git_flat() {
        let mut app = make_app();
        app.git_mode = true;
        app.git_mode_flat = true;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("[git:flat]"));
    }

    #[test]
    fn content_focus_default() {
        let mut app = make_app();
        app.focus = Focus::Content;
        let text = render_bar(&app);
        assert!(text.contains("PgUp/PgDn"));
        assert!(text.contains("H history"));
    }

    #[test]
    fn content_markdown_raw() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.word_wrap = true;
        app.is_markdown = true;
        app.show_raw_markdown = true;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("M render"));
    }

    #[test]
    fn content_markdown_rendered() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.word_wrap = true;
        app.is_markdown = true;
        app.show_raw_markdown = false;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("M raw"));
    }

    #[test]
    fn content_json_pretty() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.word_wrap = true;
        app.is_json = true;
        app.show_pretty_json = true;
        app.json_pretty_lines = vec![vec![]];
        let text = render_bar_width(&app, 120);
        assert!(text.contains("J raw"));
    }

    #[test]
    fn content_json_raw() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.is_json = true;
        app.show_pretty_json = false;
        app.json_pretty_lines = vec![vec![]];
        let text = render_bar_width(&app, 120);
        assert!(text.contains("J pretty"));
    }

    #[test]
    fn content_word_wrap() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.word_wrap = true;
        let text = render_bar(&app);
        assert!(text.contains("z no-wrap"));
        assert!(!text.contains("h-scroll"));
    }

    #[test]
    fn content_no_wrap_hscroll() {
        let mut app = make_app();
        app.focus = Focus::Content;
        app.word_wrap = false;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("z wrap"));
        assert!(text.contains("h-scroll"));
    }

    #[test]
    fn scroll_percentage_shown() {
        let mut app = make_app();
        app.show_scroll_percentage = true;
        app.current_file = Some(PathBuf::from("Cargo.toml"));
        app.content = vec!["x".to_string(); 50];
        app.content_area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 10,
        };
        app.content_scroll = 25;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("%"));
    }

    #[test]
    fn scroll_percentage_no_file() {
        let mut app = make_app();
        app.show_scroll_percentage = true;
        app.current_file = None;
        let text = render_bar_width(&app, 120);
        assert!(!text.contains("%"));
    }

    #[test]
    fn git_info_clean() {
        let mut app = make_app();
        app.git_info = Some(GitRepoInfo {
            head: GitHead::Branch("main".into()),
            ahead: 0,
            behind: 0,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        });
        let text = render_bar_width(&app, 120);
        assert!(text.contains("[main]"));
    }

    #[test]
    fn git_info_dirty() {
        let mut app = make_app();
        app.git_info = Some(GitRepoInfo {
            head: GitHead::Branch("feature".into()),
            ahead: 3,
            behind: 2,
            total_changed: 5,
            staged: 1,
            untracked: 4,
        });
        let text = render_bar_width(&app, 120);
        assert!(text.contains("\u{2191}3"));
        assert!(text.contains("\u{2193}2"));
        assert!(text.contains("+5"));
    }

    #[test]
    fn git_info_detached() {
        let mut app = make_app();
        app.git_info = Some(GitRepoInfo {
            head: GitHead::Detached,
            ahead: 0,
            behind: 0,
            total_changed: 1,
            staged: 1,
            untracked: 0,
        });
        let text = render_bar_width(&app, 120);
        assert!(text.contains("HEAD (detached)"));
    }

    #[test]
    fn config_error_displayed() {
        let mut app = make_app();
        app.config_error = Some("bad config".into());
        let text = render_bar_width(&app, 120);
        assert!(text.contains("config error"));
    }

    #[test]
    fn walk_errors_displayed() {
        let mut app = make_app();
        app.walk_errors = 3;
        let text = render_bar_width(&app, 120);
        assert!(text.contains("[!3]"));
    }

    #[test]
    fn git_info_str_clean_branch() {
        let info = GitRepoInfo {
            head: GitHead::Branch("main".into()),
            ahead: 0,
            behind: 0,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [main]");
    }

    #[test]
    fn git_info_str_dirty_branch() {
        let info = GitRepoInfo {
            head: GitHead::Branch("fix".into()),
            ahead: 1,
            behind: 2,
            total_changed: 3,
            staged: 1,
            untracked: 2,
        };
        assert_eq!(git_info_str(&info), " [fix ↑1 ↓2 +3]");
    }

    #[test]
    fn git_info_str_detached_head() {
        let info = GitRepoInfo {
            head: GitHead::Detached,
            ahead: 0,
            behind: 0,
            total_changed: 1,
            staged: 1,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [HEAD (detached) +1]");
    }

    #[test]
    fn git_info_str_rebase() {
        let info = GitRepoInfo {
            head: GitHead::Rebase,
            ahead: 0,
            behind: 0,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [REBASE]");
    }

    #[test]
    fn git_info_str_merge() {
        let info = GitRepoInfo {
            head: GitHead::Merge,
            ahead: 0,
            behind: 0,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [MERGE]");
    }

    #[test]
    fn git_info_str_ahead_only() {
        let info = GitRepoInfo {
            head: GitHead::Branch("main".into()),
            ahead: 5,
            behind: 0,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [main ↑5]");
    }

    #[test]
    fn git_info_str_behind_only() {
        let info = GitRepoInfo {
            head: GitHead::Branch("main".into()),
            ahead: 0,
            behind: 3,
            total_changed: 0,
            staged: 0,
            untracked: 0,
        };
        assert_eq!(git_info_str(&info), " [main ↓3]");
    }
}
