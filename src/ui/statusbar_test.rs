use super::*;
use crate::app::{App, Focus};
use crate::config::Config;
use crate::git::{GitHead, GitRepoInfo};
use crate::search::{GotoLineState, HistoryState, SearchState, ThemePicker};
use ratatui::backend::TestBackend;
use ratatui::style::Style;
use ratatui::text::Span;
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
    render_bar_width(app, 200)
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
fn hint_goto_line() {
    let mut app = make_app();
    app.goto_line = Some(GotoLineState::new());
    let text = render_bar(&app);
    assert!(text.contains("Enter jump"));
    assert!(text.contains("+N forward"));
    assert!(text.contains("-N back"));
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
fn visual_line_hint_shown() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.visual_line = Some(crate::selection::VisualLine::new(0));
    let text = render_bar_width(&app, 120);
    assert!(text.contains("VISUAL LINE"));
    assert!(text.contains("b blame"));
    assert!(text.contains("Esc exit"));
}

#[test]
fn visual_line_hint_shows_hide_blame_when_panel_open() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.visual_line = Some(crate::selection::VisualLine::new(0));
    app.blame_panel = true;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("b hide blame"));
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
#[test]
fn watch_badge_shown_when_active() {
    let mut app = make_app();
    app.auto_watch = true;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[watch]"));
}

#[test]
fn watch_badge_hidden_when_inactive() {
    let app = make_app();
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[watch]"));
}

#[test]
fn file_info_shows_encoding_and_line_ending() {
    let mut app = make_app();
    app.show_file_info = true;
    app.file_encoding = Some("UTF-8".to_string());
    app.file_line_ending = Some("LF".to_string());
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[UTF-8 LF]"));
}

#[test]
fn file_info_shows_encoding_without_line_ending() {
    let mut app = make_app();
    app.show_file_info = true;
    app.file_encoding = Some("ASCII".to_string());
    app.file_line_ending = None;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[ASCII]"));
    assert!(!text.contains("[ASCII "));
}

#[test]
fn file_info_hidden_when_toggled_off() {
    let mut app = make_app();
    app.show_file_info = false;
    app.file_encoding = Some("UTF-8".to_string());
    app.file_line_ending = Some("LF".to_string());
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[UTF-8 LF]"));
}

#[test]
fn file_info_mixed_endings_shown() {
    let mut app = make_app();
    app.show_file_info = true;
    app.file_encoding = Some("UTF-8".to_string());
    app.file_line_ending = Some("mixed".to_string());
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[UTF-8 mixed]"));
}

#[test]
fn ln_shown_when_file_open() {
    let mut app = make_app();
    app.current_file = Some(PathBuf::from("Cargo.toml"));
    app.active_line = 10;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("Ln 11"));
    assert!(!text.contains("Col"));
}

#[test]
fn ln_col_hidden_when_no_file() {
    let mut app = make_app();
    app.current_file = None;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("Ln "));
}

#[test]
fn ln_col_hidden_when_diff() {
    let mut app = make_app();
    app.current_file = Some(PathBuf::from("file.patch"));
    app.is_diff = true;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("Ln "));
}

#[test]
fn syntax_name_shown_when_present() {
    let mut app = make_app();
    app.current_file = Some(PathBuf::from("main.rs"));
    app.current_syntax = Some("Rust".to_string());
    app.active_line = 0;
    app.content_hscroll = 0;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[Rust]"));
}

#[test]
fn syntax_name_hidden_when_none() {
    let mut app = make_app();
    app.current_file = Some(PathBuf::from("plain.txt"));
    app.current_syntax = None;
    let text = render_bar_width(&app, 120);
    // No syntax badge appears; only Ln is shown.
    assert!(text.contains("Ln 1"));
    assert!(!text.contains("Col")); // no Col indicator
    assert!(!text.contains("plain")); // no "[plain]" badge
}

// ── Narrow-terminal elision tests ────────────────────────────────────────

#[test]
fn narrow5_shows_empty_bar() {
    let app = make_app();
    let text = render_bar_width(&app, 5);
    // At width 5 even the version string (" v0.7.12", 8 chars) // matches RELEASE.version can't fit.
    let trimmed = text.trim_end_matches(' ');
    assert!(
        trimmed.is_empty(),
        "bar should be empty at width 5, got {trimmed:?}"
    );
}

#[test]
fn narrow20_keeps_version_drops_hint() {
    let app = make_app();
    let text = render_bar_width(&app, 20);
    assert!(text.contains("v0.7.12"), "version should fit at width 20");
    assert!(
        !text.contains("j/k"),
        "keybinding hint should be elided at width 20"
    );
}

#[test]
fn narrow_keeps_errors_over_badges() {
    let mut app = make_app();
    app.show_hidden = true;
    app.walk_errors = 3;
    // Version " v0.7.12" (8) + " [!3]" (5) = 13; " [hidden]" (9) = 22 total.
    // At width 19, one must go; [hidden] (P_INFO=2) has lower priority than
    // [!3] (P_ERR=4) and version (P_VER=5).
    let text = render_bar_width(&app, 19);
    assert!(text.contains("[!3]"), "errors should be kept at width 19");
    assert!(text.contains("v0.7.12"), "version should be kept");
    assert!(!text.contains("[hidden]"), "badges elided before errors");
}

#[test]
fn narrow_keeps_git_over_badges() {
    let mut app = make_app();
    app.show_hidden = true;
    app.git_info = Some(GitRepoInfo {
        head: GitHead::Branch("main".into()),
        ahead: 2,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    // Total: hint (92) + [hidden] (9) + git (9) + version (7) = 117.
    // At width 20: drop hint → 25 (still > 20), drop [hidden] (P_INFO=2) → 16 (≤20).
    // Git (P_GIT=3) and version (P_VER=5) stay.
    let text = render_bar_width(&app, 20);
    assert!(text.contains("[main ↑2]"), "git info kept at width 20");
    assert!(text.contains("v0.7.12"), "version kept");
    assert!(!text.contains("[hidden]"), "badge elided before git info");
}

#[test]
fn narrow_keeps_git_info() {
    let mut app = make_app();
    app.git_info = Some(GitRepoInfo {
        head: GitHead::Branch("main".into()),
        ahead: 1,
        behind: 2,
        total_changed: 3,
        staged: 1,
        untracked: 2,
    });
    let text = render_bar_width(&app, 45);
    assert!(
        text.contains("[main ↑1 ↓2 +3]"),
        "git info kept at width 45"
    );
    assert!(text.contains("v0.7.12"), "version kept");
}

#[test]
fn narrow_keeps_config_error() {
    let mut app = make_app();
    app.config_error = Some("bad".into());
    let text = render_bar_width(&app, 30);
    assert!(
        text.contains("[config error]"),
        "config error kept at width 30"
    );
    assert!(text.contains("v0.7.12"), "version kept");
}

#[test]
fn narrow_drops_meta_before_badges() {
    let mut app = make_app();
    app.status_message = Some("hello".into());
    app.show_hidden = true;
    // Total with all items: hint (~92) + [hidden] (9) + hello (6) + version (7) = 114.
    // At width 20: drop hint → 22 (still > 20), then drop hello (P_META=1) → 16 (≤20).
    // [hidden] (P_INFO=2) and version stay.
    let text = render_bar_width(&app, 20);
    assert!(
        !text.contains("hello"),
        "status message (P_META) should be elided before badges"
    );
    assert!(text.contains("v0.7.12"), "version kept");
}

#[test]
fn overlay_theme_picker_truncated_at_narrow_width() {
    let mut app = make_app();
    app.theme_picker = Some(ThemePicker::default());
    let text = render_bar_width(&app, 20);
    let char_count = text.chars().count();
    assert!(char_count == 20, "bar has {char_count} chars at width 20");
    // Full hint is ~65 chars; at width 20 it should be truncated with «…».
    let trimmed = text.trim_end_matches(' ');
    assert_eq!(
        trimmed.chars().last(),
        Some('…'),
        "overlay hint should end with ellipsis"
    );
}

#[test]
fn fit_segments_empty_input() {
    let line = fit_segments(vec![], 80);
    assert_eq!(line.width(), 0);
}

#[test]
fn fit_segments_zero_max_width() {
    let segs = vec![(Span::styled("hello", Style::default()), P_HINT)];
    let line = fit_segments(segs, 0);
    assert_eq!(line.width(), 0);
}

#[test]
fn bar_never_overflows_various_widths() {
    let mut app = make_app();
    app.show_hidden = true;
    app.git_mode = true;
    app.auto_watch = true;
    app.walk_errors = 2;
    app.git_info = Some(GitRepoInfo {
        head: GitHead::Branch("main".into()),
        ahead: 3,
        behind: 1,
        total_changed: 5,
        staged: 2,
        untracked: 3,
    });
    for width in [5, 10, 15, 20, 25, 30, 40, 50, 60] {
        let text = render_bar_width(&app, width);
        // The rendered line should never show unintended clipping — no
        // content that starts beyond the terminal width.
        let char_count = text.chars().count();
        assert!(
            char_count == width as usize,
            "bar has {char_count} chars at width {width}: {text:?}",
        );
    }
}
