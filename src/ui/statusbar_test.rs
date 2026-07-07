use super::*;
use crate::app::{App, Focus, StatusMessage};
use crate::config::Config;
use crate::git::{GitHead, GitRepoInfo};
use crate::search::{GotoLineState, HistoryState, SearchState, ThemePicker};
use ratatui::backend::TestBackend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::Terminal;
use std::path::{Path, PathBuf};

fn make_app() -> App {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
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
    app.search = Some(SearchState::new(Path::new("."), false, true, 0, None));
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
fn tree_focus_no_keybinding_hint() {
    let app = make_app();
    let text = render_bar(&app);
    assert!(!text.contains("j/k nav"), "keybinding hint was removed");
    assert!(text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
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
fn tree_focus_compare_badge_shows_revision() {
    let mut app = make_app();
    app.git_mode = true;
    app.compare_base = Some("HEAD~3".to_string());
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[compare: HEAD~3]"));
}

#[test]
fn tree_focus_compare_badge_hidden_when_not_comparing() {
    let mut app = make_app();
    app.git_mode = true;
    app.compare_base = None;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[compare:"));
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
fn content_focus_no_keybinding_hint() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.is_json = true;
    app.json_pretty_lines = vec![vec![]];
    let text = render_bar(&app);
    assert!(!text.contains("PgUp/PgDn"), "keybinding hint was removed");
    assert!(!text.contains("H history"));
    assert!(!text.contains("J pretty"));
    assert!(!text.contains("z wrap"));
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

#[test]
fn diff_mode_badge_shown_when_diff_and_git_mode() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.is_diff = true;
    app.git_mode = true;
    app.diff_mode = crate::app::DiffMode::Staged;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[diff: staged"));
}

#[test]
fn diff_mode_badge_uses_diff_mode_label() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.is_diff = true;
    app.git_mode = true;
    app.diff_mode = crate::app::DiffMode::Unstaged;
    let text = render_bar_width(&app, 120);
    assert!(text.contains("[diff: unstaged"));
}

#[test]
fn diff_mode_badge_hidden_when_not_diff() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.is_diff = false;
    app.git_mode = true;
    app.diff_mode = crate::app::DiffMode::All;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[diff:"));
}

#[test]
fn diff_mode_badge_hidden_when_not_git_mode() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.is_diff = true;
    app.git_mode = false;
    app.diff_mode = crate::app::DiffMode::All;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[diff:"));
}

#[test]
fn diff_mode_badge_hidden_when_neither_diff_nor_git_mode() {
    let mut app = make_app();
    app.is_diff = false;
    app.git_mode = false;
    let text = render_bar_width(&app, 120);
    assert!(!text.contains("[diff:"));
}

// ── Narrow-terminal elision tests ────────────────────────────────────────

#[test]
fn narrow5_shows_empty_bar() {
    let app = make_app();
    let text = render_bar_width(&app, 5);
    // At width 5 even the version string can't fit.
    let trimmed = text.trim_end_matches(' ');
    assert!(
        trimmed.is_empty(),
        "bar should be empty at width 5, got {trimmed:?}"
    );
}

#[test]
fn narrow20_keeps_version() {
    let app = make_app();
    let text = render_bar_width(&app, 20);
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version should fit at width 20"
    );
}

#[test]
fn narrow_keeps_errors_over_badges() {
    let mut app = make_app();
    app.show_hidden = true;
    app.walk_errors = 3;
    // Version " vX.Y.Z" (8) + " [!3]" (5) = 13; " [hidden]" (9) = 22 total.
    // At width 19, one must go; [hidden] (P_INFO=2) has lower priority than
    // [!3] (P_ERR=4) and version (P_VER=5).
    let text = render_bar_width(&app, 19);
    assert!(text.contains("[!3]"), "errors should be kept at width 19");
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version should be kept"
    );
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
    // Total: [hidden] (9) + git (9) + version (7) = 25.
    // At width 20: drop [hidden] (P_INFO=2) → 16 (≤20).
    // Git (P_GIT=3) and version (P_VER=5) stay.
    let text = render_bar_width(&app, 20);
    assert!(text.contains("[main ↑2]"), "git info kept at width 20");
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version kept"
    );
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
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version kept"
    );
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
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version kept"
    );
}

#[test]
fn narrow_drops_meta_before_badges() {
    let mut app = make_app();
    app.status_message = Some(StatusMessage::new("hello", std::time::Instant::now()));
    app.show_hidden = true;
    // Total: [hidden] (9) + hello (6) + version (7) = 22.
    // At width 20: drop hello (P_META=1) → 16 (≤20).
    // [hidden] (P_INFO=2) and version stay.
    let text = render_bar_width(&app, 20);
    assert!(
        !text.contains("hello"),
        "status message (P_META) should be elided before badges"
    );
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version kept"
    );
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
    let cfg = StatusBarConfig::default();
    let line = fit_two_sided(vec![], 80, &cfg);
    assert_eq!(line.width(), 0);
}

#[test]
fn fit_segments_zero_max_width() {
    let cfg = StatusBarConfig::default();
    let segs = vec![(
        Span::styled("hello", Style::default()),
        StatusSegment::Badges,
        P_INFO,
    )];
    let line = fit_two_sided(segs, 0, &cfg);
    assert_eq!(line.width(), 0);
}

#[test]
fn fold_stats_without_anchors() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.fold_regions = vec![
        crate::fold::FoldRegion { start: 0, end: 3 },
        crate::fold::FoldRegion { start: 5, end: 8 },
    ];
    app.folded.insert(0);
    let text = render_bar(&app);
    assert!(text.contains("[1/2]"), "expected [1/2] in {text:?}");
}

#[test]
fn fold_stats_with_yaml_anchors() {
    let mut app = make_app();
    app.focus = Focus::Content;
    app.fold_regions = vec![crate::fold::FoldRegion { start: 0, end: 3 }];
    app.yaml_anchor_count = 2;
    app.yaml_alias_count = 1;
    let text = render_bar(&app);
    // Anchor suffix must be separated from the fold count.
    assert!(
        text.contains("[&2 *1 0/1]"),
        "expected [&2 *1 0/1] in {text:?}"
    );
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

// ── Explicit-allowlist / empty-bar tests ──────────────────────────────────

#[test]
fn both_none_default_split() {
    // Regression: both None should behave like the old default.
    let cfg = StatusBarConfig {
        left: None,
        right: None,
    };
    let segs = vec![
        (
            Span::styled("badges", Style::default()),
            StatusSegment::Badges,
            P_INFO,
        ),
        (
            Span::styled("ver", Style::default()),
            StatusSegment::Version,
            P_VER,
        ),
        (
            Span::styled("git", Style::default()),
            StatusSegment::Git,
            P_GIT,
        ),
    ];
    let (left, right) = split_sides(segs, &cfg);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].content.as_ref(), "badges");
    assert_eq!(right.len(), 2);
    assert_eq!(right[0].content.as_ref(), "ver");
    assert_eq!(right[1].content.as_ref(), "git");
}

#[test]
fn both_some_empty_yields_empty_bar() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            left: Some(vec![]),
            right: Some(vec![]),
        },
        ..Config::default()
    };
    let app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    let text = render_bar_width(&app, 80);
    let trimmed = text.trim_end_matches(' ');
    assert!(
        trimmed.is_empty(),
        "bar should be empty with both allowlists empty, got {text:?}"
    );
}

#[test]
fn explicit_left_right_only_listed_segments_render() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            left: Some(vec!["badges".into()]),
            right: Some(vec!["version".into()]),
        },
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    // Enable show_hidden and auto_watch so badges and (unlisted) message
    // segments have content.
    app.show_hidden = true;
    app.status_message = Some(StatusMessage::new("hello", std::time::Instant::now()));
    let text = render_bar_width(&app, 200);
    // Badges on left.
    assert!(text.contains("[hidden]"), "badges should be visible");
    // Version on right.
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        text.trim_end().ends_with(&version),
        "version should be right-aligned, got {text:?}"
    );
    // Message segment not in allowlist → must not appear.
    assert!(
        !text.contains("hello"),
        "message should be hidden in explicit mode, got {text:?}"
    );
}

#[test]
fn explicit_elision_still_drops_lowest_priority() {
    let mut app = {
        let cfg = Config {
            git: crate::config::GitConfig {
                status: false,
                ..Default::default()
            },
            statusbar: StatusBarConfig {
                left: Some(vec!["badges".into()]),
                right: Some(vec!["version".into()]),
            },
            ..Config::default()
        };
        App::new(PathBuf::from("."), cfg, None, None).unwrap()
    };
    app.show_hidden = true;
    // Visible: "[hidden]"(9) + " vX.Y.Z"(~8) = ~17 total.
    // At width 8, neither fits; elision drops [hidden] (P_INFO=2) first,
    // leaving version (P_VER=5) — but version alone is ~8 chars, might barely
    // not fit either. Use width 14: still < 17, drops [hidden], version stays.
    let text = render_bar_width(&app, 14);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    assert!(
        text.contains(&version),
        "version (highest priority) should survive elision at width 14, got {text:?}"
    );
    assert!(
        !text.contains("[hidden]"),
        "badges elided before version at width 14, got {text:?}"
    );
}

// ── Alignment tests ───────────────────────────────────────────────────────

#[test]
fn default_config_left_segment_starts_at_col_0() {
    let mut app = make_app();
    app.show_hidden = true;
    let text = render_bar_width(&app, 200);
    assert!(
        text.starts_with(" [hidden]"),
        "first left segment should start at column 0, got {text:?}"
    );
}

#[test]
fn default_config_version_right_aligned() {
    let app = make_app();
    let text = render_bar_width(&app, 200);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // Version should appear near the right end, after padding spaces.
    assert!(
        text.trim_end().ends_with(&version),
        "version should be right-aligned, got {text:?}"
    );
}

#[test]
fn default_config_lnum_right_side() {
    let mut app = make_app();
    app.current_file = Some(PathBuf::from("Cargo.toml"));
    app.active_line = 10;
    let text = render_bar_width(&app, 200);
    // Ln 11 should be in the right-anchored block, past the padding gap.
    let lnum_pos = text.find("Ln 11").unwrap();
    assert!(
        lnum_pos > 100,
        "Ln should be right-aligned, got Ln at {lnum_pos}"
    );
}

#[test]
fn wide_bar_left_plus_gap_plus_right_equals_max() {
    let mut app = make_app();
    app.show_hidden = true;
    app.git_info = Some(GitRepoInfo {
        head: GitHead::Branch("main".into()),
        ahead: 0,
        behind: 0,
        total_changed: 0,
        staged: 0,
        untracked: 0,
    });
    let text = render_bar_width(&app, 200);
    assert_eq!(text.chars().count(), 200);
}

#[test]
fn narrow_bar_drops_lowest_priority_across_both_sides() {
    let mut app = make_app();
    app.walk_errors = 1;
    let text = render_bar_width(&app, 15);
    // At width 15 only version (" vX.Y.Z") and error (" [!1]") should survive.
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version should be kept at width 15"
    );
    assert!(text.contains("[!1]"), "errors should survive at width 15");
}

#[test]
fn custom_right_only_version() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            right: Some(vec!["version".into()]),
            left: Some(vec!["badges".into()]),
        },
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    app.show_hidden = true;
    let text = render_bar_width(&app, 200);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // Version should be the only right-side segment, at the end.
    assert!(
        text.trim_end().ends_with(&version),
        "version should be right-aligned, got {text:?}"
    );
    // Badges on left, version on right, everything else hidden.
    assert!(
        text.contains("[hidden]"),
        "badges should be visible on left"
    );
    assert!(
        !text.contains("[main]"),
        "git segment should be hidden in explicit mode"
    );
}

#[test]
fn empty_right_all_left() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            left: None,
            right: None,
        },
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    app.show_hidden = true;
    let text = render_bar_width(&app, 200);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // In default mode (both None), version is on the right, badges on left.
    assert!(text.starts_with(" [hidden]"));
    assert!(text.trim_end().ends_with(&version));
}

#[test]
fn nonexistent_id_ignored() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            left: Some(vec!["badges".into()]),
            right: Some(vec!["nonexistent".into()]),
        },
        ..Config::default()
    };
    let mut app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    app.show_hidden = true;
    let text = render_bar_width(&app, 200);
    // "nonexistent" doesn't match any built segment; only badges (left) appear.
    assert!(text.starts_with(" [hidden]"));
    assert!(!text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))));
}

#[test]
fn split_sides_default_mode_natural_order() {
    // Default mode: both None.
    let cfg = StatusBarConfig {
        left: None,
        right: None,
    };
    // Manually exercise split_sides.
    let segs = vec![
        (
            Span::styled("badges", Style::default()),
            StatusSegment::Badges,
            P_INFO,
        ),
        (
            Span::styled("ver", Style::default()),
            StatusSegment::Version,
            P_VER,
        ),
        (
            Span::styled("git", Style::default()),
            StatusSegment::Git,
            P_GIT,
        ),
    ];
    let (left, right) = split_sides(segs, &cfg);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].content.as_ref(), "badges");
    assert_eq!(right.len(), 2);
    assert_eq!(right[0].content.as_ref(), "ver");
    assert_eq!(right[1].content.as_ref(), "git");
}

#[test]
fn split_sides_explicit_mode_order_follows_config() {
    let cfg = StatusBarConfig {
        left: Some(vec!["git".into(), "badges".into()]),
        right: Some(vec!["version".into()]),
    };
    let segs = vec![
        (
            Span::styled("badges", Style::default()),
            StatusSegment::Badges,
            P_INFO,
        ),
        (
            Span::styled("ver", Style::default()),
            StatusSegment::Version,
            P_VER,
        ),
        (
            Span::styled("git", Style::default()),
            StatusSegment::Git,
            P_GIT,
        ),
    ];
    let (left, right) = split_sides(segs, &cfg);
    // Left order follows config: git before badges.
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].content.as_ref(), "git");
    assert_eq!(left[1].content.as_ref(), "badges");
    // Right: version.
    assert_eq!(right.len(), 1);
    assert_eq!(right[0].content.as_ref(), "ver");
}

#[test]
fn compose_left_right_padding() {
    let left = vec![Span::styled("left", Style::default())];
    let right = vec![Span::styled("right", Style::default())];
    // left=4, right=5, max=20 -> gap=11
    let line = compose_left_right(left, right, 20);
    assert_eq!(line.width(), 20);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "left           right");
}

#[test]
fn compose_left_right_no_padding_needed() {
    let left = vec![Span::styled("abc", Style::default())];
    let right = vec![Span::styled("de", Style::default())];
    let line = compose_left_right(left, right, 7);
    assert_eq!(line.width(), 7);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "abc  de");
}

#[test]
fn compose_left_right_exact_fit() {
    let left = vec![Span::styled("abc", Style::default())];
    let right = vec![Span::styled("de", Style::default())];
    let line = compose_left_right(left, right, 5);
    assert_eq!(line.width(), 5);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "abcde");
}

#[test]
fn compose_left_right_empty_groups() {
    let line = compose_left_right(vec![], vec![], 10);
    assert_eq!(line.width(), 10);
    let content: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    assert_eq!(content, "          ");
}

#[test]
fn git_info_none_no_branch_segment() {
    // git_info is now the sole git source (plugin_git_info removed in 0.11.22).
    // With git_info = None, no branch or git-status segment renders.
    let app = make_app(); // git.status = false → git_info stays None
    assert!(app.git_info.is_none());
    let text = render_bar(&app);
    assert!(!text.contains("[main]"));
    assert!(!text.contains("HEAD (detached)"));
}

// ── Right-edge flush with Unicode glyphs (Part 2) ────────────────────────

#[test]
fn right_segment_with_unicode_arrows_is_flush_to_edge() {
    // Render a bar with git info containing ↑↓ glyphs, verify the last
    // non-space character hits the right edge.
    let mut app = make_app();
    app.git_info = Some(GitRepoInfo {
        head: GitHead::Branch("main".into()),
        ahead: 3,
        behind: 2,
        total_changed: 5,
        staged: 2,
        untracked: 3,
    });
    // width = 200 is generous: everything fits, right group is anchored flush.
    let text = render_bar_width(&app, 200);
    // The bar should be exactly 200 chars wide.
    assert_eq!(text.chars().count(), 200);
    // Rightmost segment (version) must be at the very end, non-space.
    let trimmed = text.trim_end_matches(' ');
    assert!(
        trimmed.ends_with(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version (rightmost segment) should be flush to right edge, got {trimmed:?}"
    );
    // Git arrow glyphs should not cause an off-by-one gap.
    assert!(text.contains('\u{2191}'), "↑ should appear");
    assert!(text.contains('\u{2193}'), "↓ should appear");
}

#[test]
fn right_segment_flush_at_various_widths() {
    // At any width, after compose_left_right the right group must sit flush
    // to the right edge (no phantom gap after the last segment).
    let app = make_app();
    for width in [30, 40, 50, 60, 80, 100] {
        let text = render_bar_width(&app, width);
        let last_non_space = text.trim_end_matches(' ').chars().last().unwrap_or(' ');
        assert_ne!(
            last_non_space, ' ',
            "bar at width {width} should not have trailing space after last segment"
        );
    }
}

#[test]
fn plugin_error_renders_in_bar() {
    let mut app = make_app();
    app.plugin_error = Some("[markdown] failed to render (on_file_open)".to_string());
    let text = render_bar(&app);
    assert!(
        text.contains("[markdown] failed to render (on_file_open)"),
        "plugin_error text should appear in the status bar, got: {text:?}"
    );
}

#[test]
fn plugin_error_absent_by_default() {
    let app = make_app();
    assert!(app.plugin_error.is_none());
    let text = render_bar(&app);
    assert!(!text.contains("markdown"), "no plugin_error means no badge");
}

#[test]
fn plugin_error_outlasts_status_message_on_narrow_bar() {
    // plugin_error (P_ERR) must survive eliding alongside other error
    // indicators even when a lower-priority status_message (P_META) does not.
    let mut app = make_app();
    app.status_message = Some(StatusMessage::new("hello", std::time::Instant::now()));
    app.plugin_error = Some("boom".to_string());
    let text = render_bar_width(&app, 15);
    assert!(
        !text.contains("hello"),
        "status message (P_META) should be elided before plugin_error"
    );
    assert!(
        text.contains("boom"),
        "plugin_error (P_ERR) must survive eliding, got: {text:?}"
    );
}

#[test]
fn monochrome_theme_uses_reversed_modifier_not_bg_color() {
    let mut app = make_app();
    app.theme = crate::theme::Theme::monochrome();
    let backend = TestBackend::new(20, 1);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_statusbar(f, &app, f.area()))
        .unwrap();
    let buf = terminal.backend().buffer();
    let cell = &buf[(0, 0)];
    assert_eq!(
        cell.bg,
        Color::Reset,
        "monochrome bar must not set a bg color"
    );
    assert!(
        cell.modifier.contains(Modifier::REVERSED),
        "monochrome bar must use REVERSED instead of a background color"
    );
}

#[test]
fn update_notice_renders_in_bar_when_newer_version_available() {
    let mut app = make_app();
    app.new_version_available = Some("v99.0.0".to_string());
    let text = render_bar(&app);
    assert!(
        text.contains("v99.0.0"),
        "update notice should appear in the status bar, got: {text:?}"
    );
}

#[test]
fn update_notice_absent_by_default() {
    let app = make_app();
    assert!(app.new_version_available.is_none());
    let text = render_bar(&app);
    assert!(
        !text.contains("update:"),
        "no notice means no update segment"
    );
}
