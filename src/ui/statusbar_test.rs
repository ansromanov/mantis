use super::*;
use crate::app::{App, Focus, StatusMessage};
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
fn narrow20_keeps_version_drops_hint() {
    let app = make_app();
    let text = render_bar_width(&app, 20);
    assert!(
        text.contains(&format!("v{}", env!("CARGO_PKG_VERSION"))),
        "version should fit at width 20"
    );
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
    // Total: hint (92) + [hidden] (9) + git (9) + version (7) = 117.
    // At width 20: drop hint → 25 (still > 20), drop [hidden] (P_INFO=2) → 16 (≤20).
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
    // Total with all items: hint (~92) + [hidden] (9) + hello (6) + version (7) = 114.
    // At width 20: drop hint → 22 (still > 20), then drop hello (P_META=1) → 16 (≤20).
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
        StatusSegment::Hint,
        P_HINT,
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

// ── Alignment tests ───────────────────────────────────────────────────────

#[test]
fn default_config_hint_starts_at_col_0() {
    let app = make_app();
    let text = render_bar_width(&app, 200);
    assert!(
        text.starts_with(" j/k nav"),
        "hint should start at column 0, got {text:?}"
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
    // Ln 11 should be on the right side, after padding
    let hint_end = text.find("? help").unwrap();
    let lnum_pos = text.find("Ln 11").unwrap();
    assert!(
        lnum_pos > hint_end,
        "Ln should be right-aligned after hint, got Ln at {lnum_pos}, hint ends at {hint_end}"
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
            right: vec!["version".into()],
        },
        ..Config::default()
    };
    let app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    let text = render_bar_width(&app, 200);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // Version should be the only right-side segment, at the end.
    assert!(
        text.trim_end().ends_with(&version),
        "version should be right-aligned, got {text:?}"
    );
    // Ln shouldn't appear since no file is open, but git/type/lnum
    // are not in right list so they fall to the left.
}

#[test]
fn empty_right_all_left() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig { right: vec![] },
        ..Config::default()
    };
    let app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    let text = render_bar_width(&app, 200);
    let version = format!("v{}", env!("CARGO_PKG_VERSION"));
    // Version is now left-aligned — should appear near the right end but
    // without any padding-gap before it; should be immediately after earlier segments.
    // Just verify the bar starts with the hint and ends with version without extra gap.
    assert!(text.starts_with(" j/k nav"));
    assert!(text.contains(&version));
}

#[test]
fn unknown_id_in_right_ignored() {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        statusbar: StatusBarConfig {
            right: vec!["nonexistent".into()],
        },
        ..Config::default()
    };
    let app = App::new(PathBuf::from("."), cfg, None, None).unwrap();
    let text = render_bar_width(&app, 200);
    // With default config settings, nothing is in right list (since "nonexistent"
    // doesn't match any segment), so all should be left-aligned.
    assert!(text.starts_with(" j/k nav"));
}

#[test]
fn split_sides_left_right_groups() {
    let cfg = StatusBarConfig {
        right: vec!["version".into(), "git".into()],
    };
    // Manually exercise split_sides.
    let segs = vec![
        (
            Span::styled("hint", Style::default()),
            StatusSegment::Hint,
            P_HINT,
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
    assert_eq!(left[0].content.as_ref(), "hint");
    assert_eq!(right.len(), 2);
    assert_eq!(right[0].content.as_ref(), "ver");
    assert_eq!(right[1].content.as_ref(), "git");
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
