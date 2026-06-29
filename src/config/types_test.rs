use super::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};

#[test]
fn git_show_untracked_defaults_to_true() {
    let cfg = Config::default();
    assert!(cfg.git.show_untracked);
}

#[test]
fn git_show_untracked_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            show_untracked: false,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(!parsed.git.show_untracked);
}

#[test]
fn git_show_ignored_defaults_to_false() {
    let cfg = Config::default();
    assert!(!cfg.git.show_ignored);
}

#[test]
fn git_show_ignored_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            show_ignored: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(parsed.git.show_ignored);
}

#[test]
fn icons_defaults_to_false() {
    let cfg = Config::default();
    assert!(!cfg.tree.icons);
}

#[test]
fn icons_round_trips_through_serde() {
    let cfg = Config {
        tree: TreeConfig {
            icons: true,
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(parsed.tree.icons);
}

#[test]
fn icons_false_round_trips_through_serde() {
    let cfg = Config::default();
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert!(!parsed.tree.icons);
}

// -- find_files keybinding ---------------------------------------------------

#[test]
fn find_files_defaults_to_ctrl_f() {
    let cfg = Config::default();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "default find_files must include ctrl+f"
    );
}

#[test]
fn find_files_can_be_overridden_in_config() {
    let toml_str = r#"
[keys]
find_files = ["ctrl+x"]
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('x'), KeyModifiers::CONTROL)) }),
        "overridden find_files must match ctrl+x"
    );
    assert!(
        !cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "overridden find_files must not include ctrl+f"
    );
}

#[test]
fn find_files_without_config_key_gets_default_from_serde_container() {
    let toml_str = r#"
[keys]
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(
        cfg.keys
            .find_files
            .iter()
            .any(|b| { b.matches(&KeyEvent::new(KeyCode::Char('f'), KeyModifiers::CONTROL)) }),
        "omitted find_files must default to ctrl+f via serde container default"
    );
}

#[test]
fn diff_mode_defaults_to_all() {
    let cfg = Config::default();
    assert_eq!(cfg.git.diff.mode, crate::app::DiffMode::All);
}

#[test]
fn diff_mode_staged_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            diff: GitDiffConfig {
                mode: crate::app::DiffMode::Staged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.git.diff.mode, crate::app::DiffMode::Staged);
}

#[test]
fn diff_mode_unstaged_round_trips_through_serde() {
    let cfg = Config {
        git: GitConfig {
            diff: GitDiffConfig {
                mode: crate::app::DiffMode::Unstaged,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.git.diff.mode, crate::app::DiffMode::Unstaged);
}

#[test]
fn legacy_flat_git_keys_fold_into_git_config() {
    let toml_str = r#"
git_status = false
ignore_gitignore = true
git_show_deleted = true
git_show_untracked = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_git_fields();
    assert!(!cfg.git.status, "legacy git_status=false should fold");
    assert!(
        cfg.git.ignore_gitignore,
        "legacy ignore_gitignore=true should fold"
    );
    assert!(
        cfg.git.show_deleted,
        "legacy git_show_deleted=true should fold"
    );
    assert!(
        !cfg.git.show_untracked,
        "legacy git_show_untracked=false should fold"
    );
}

#[test]
fn new_git_table_populates_fields() {
    let toml_str = r#"
[git]
status = false
ignore_gitignore = true
show_deleted = true
show_untracked = false

[git.diff]
mode = "staged"
side_by_side = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(!cfg.git.status);
    assert!(cfg.git.ignore_gitignore);
    assert!(cfg.git.show_deleted);
    assert!(!cfg.git.show_untracked);
    assert_eq!(cfg.git.diff.mode, crate::app::DiffMode::Staged);
    assert!(cfg.git.diff.side_by_side);
}

#[test]
fn legacy_key_wins_when_both_present() {
    // When both new [git] and legacy flat key exist, migrate folds last so
    // legacy overwrites. Legacy key must come before [git] header in TOML
    // to be at the top level (otherwise it's inside [git]).
    let toml_str = r#"
git_status = true

[git]
status = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_git_fields();
    // Legacy runs last, so git_status=true overwrites the [git] status=false.
    assert!(
        cfg.git.status,
        "legacy git_status should win over [git] status"
    );
}

#[test]
fn diff_mode_serde_round_trips() {
    #[derive(Serialize, Deserialize)]
    struct Wrap {
        m: crate::app::DiffMode,
    }
    let cases = [
        (crate::app::DiffMode::All, "m = \"all\""),
        (crate::app::DiffMode::Staged, "m = \"staged\""),
        (crate::app::DiffMode::Unstaged, "m = \"unstaged\""),
    ];
    for (mode, expected_str) in &cases {
        let toml_str = toml::to_string_pretty(&Wrap { m: *mode }).unwrap();
        assert!(toml_str.contains(expected_str), "got: {toml_str:?}");
        let wrap: Wrap = toml::from_str(expected_str).unwrap();
        assert_eq!(wrap.m, *mode);
    }
}

// ---- tree/content/search config grouping -----------------------------------

#[test]
fn legacy_flat_tree_keys_fold_into_tree_config() {
    let toml_str = r#"
show_hidden = true
tree_width = 40
tree_independent_scroll = true
indent_guides = false
icons = true
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(cfg.tree.show_hidden, "legacy show_hidden=true should fold");
    assert_eq!(cfg.tree.width, 40, "legacy tree_width=40 should fold");
    assert!(
        cfg.tree.independent_scroll,
        "legacy tree_independent_scroll=true should fold"
    );
    assert!(
        !cfg.tree.indent_guides,
        "legacy indent_guides=false should fold"
    );
    assert!(cfg.tree.icons, "legacy icons=true should fold");
}

#[test]
fn legacy_flat_content_keys_fold_into_content_config() {
    let toml_str = r#"
word_wrap = true
line_numbers = false
scrollbar = false
scroll_percentage = false
watch = true
show_file_info = false
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(cfg.content.word_wrap, "legacy word_wrap=true should fold");
    assert!(
        !cfg.content.line_numbers,
        "legacy line_numbers=false should fold"
    );
    assert!(!cfg.content.scrollbar, "legacy scrollbar=false should fold");
    assert!(
        !cfg.content.scroll_percentage,
        "legacy scroll_percentage=false should fold"
    );
    assert!(cfg.content.watch, "legacy watch=true should fold");
    assert!(
        !cfg.content.show_file_info,
        "legacy show_file_info=false should fold"
    );
}

#[test]
fn legacy_flat_search_keys_fold_into_search_config() {
    let toml_str = r#"
in_file_search = false
search_context_lines = 3
keep_search_query = true
"#;
    let mut cfg: Config = toml::from_str(toml_str).unwrap();
    cfg.migrate_legacy_flat_fields();
    assert!(
        !cfg.search.in_file_search,
        "legacy in_file_search=false should fold"
    );
    assert_eq!(
        cfg.search.context_lines, 3,
        "legacy search_context_lines=3 should fold"
    );
    assert!(
        cfg.search.keep_query,
        "legacy keep_search_query=true should fold"
    );
}

#[test]
fn new_tree_table_populates_fields() {
    let toml_str = r#"
[tree]
show_hidden = true
width = 40
independent_scroll = true
indent_guides = false
icons = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.tree.show_hidden);
    assert_eq!(cfg.tree.width, 40);
    assert!(cfg.tree.independent_scroll);
    assert!(!cfg.tree.indent_guides);
    assert!(cfg.tree.icons);
}

#[test]
fn new_content_table_populates_fields() {
    let toml_str = r#"
[content]
word_wrap = true
line_numbers = false
scrollbar = false
scroll_percentage = false
watch = true
show_file_info = false
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(cfg.content.word_wrap);
    assert!(!cfg.content.line_numbers);
    assert!(!cfg.content.scrollbar);
    assert!(!cfg.content.scroll_percentage);
    assert!(cfg.content.watch);
    assert!(!cfg.content.show_file_info);
}

#[test]
fn new_search_table_populates_fields() {
    let toml_str = r#"
[search]
in_file_search = false
context_lines = 3
keep_query = true
"#;
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert!(!cfg.search.in_file_search);
    assert_eq!(cfg.search.context_lines, 3);
    assert!(cfg.search.keep_query);
}

#[test]
fn recent_files_count_stays_top_level() {
    let toml_str = "recent_files_count = 25\n";
    let cfg: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.recent_files_count, 25);
}

#[test]
fn defaults_for_new_tables() {
    let cfg = Config::default();
    assert!(!cfg.tree.show_hidden);
    assert_eq!(cfg.tree.width, 28);
    assert!(!cfg.tree.independent_scroll);
    assert!(cfg.tree.indent_guides);
    assert!(!cfg.tree.icons);

    assert!(!cfg.content.word_wrap);
    assert!(cfg.content.line_numbers);
    assert!(cfg.content.scrollbar);
    assert!(cfg.content.scroll_percentage);
    assert!(!cfg.content.watch);
    assert!(cfg.content.show_file_info);

    assert!(cfg.search.in_file_search);
    assert_eq!(cfg.search.context_lines, 0);
    assert!(!cfg.search.keep_query);

    assert_eq!(cfg.recent_files_count, 10);
}

#[test]
fn statusbar_config_defaults_to_none() {
    let cfg = StatusBarConfig::default();
    assert!(cfg.left.is_none());
    assert!(cfg.right.is_none());
}

#[test]
fn statusbar_config_schema_has_some_fields() {
    let schema = StatusBarConfig::schema();
    assert!(schema.left.is_some());
    assert!(schema.right.is_some());
}

#[test]
fn statusbar_config_round_trips_explicit_mode() {
    let cfg = Config {
        statusbar: StatusBarConfig {
            left: Some(vec!["hint".into()]),
            right: Some(vec!["version".into()]),
        },
        ..Config::default()
    };
    let toml_str = toml::to_string_pretty(&cfg).unwrap();
    let parsed: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(parsed.statusbar.left, Some(vec!["hint".into()]));
    assert_eq!(parsed.statusbar.right, Some(vec!["version".into()]));
}
