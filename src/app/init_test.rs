//! Tests for `App::new` construction (see `init.rs`).
//!
//! These cover the directory-walk and config-driven visibility behaviour the
//! constructor is responsible for. Git-status seeding is exercised separately
//! in the git-mode tests in `mod_test.rs`.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::config::Config;

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_init_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn new_app(root: &std::path::Path, cfg: Config) -> App {
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn app_new_builds_visible_root_tree() {
    let root = temp_dir();
    fs::create_dir(root.join("sub")).unwrap();
    fs::write(root.join("a.txt"), "one\n").unwrap();
    fs::write(root.join("b.txt"), "two\n").unwrap();

    let app = new_app(&root, Config::default());

    assert_eq!(app.tree_selected, 0);
    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"a.txt"), "got {names:?}");
    assert!(names.contains(&"b.txt"), "got {names:?}");
    assert!(names.contains(&"sub"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_no_plugin_contributions() {
    // A freshly constructed App has produced no plugin output yet, so the
    // per-plugin contribution tracker must be empty.
    let root = temp_dir();
    fs::write(root.join("a.txt"), "one\n").unwrap();

    let app = new_app(&root, Config::default());

    assert!(
        app.plugin_contributions.is_empty(),
        "new App must have no plugin contributions"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_plugin_open_guard_defaults_false() {
    // The re-entrancy guard that suppresses `on_file_open` re-emission for
    // plugin-originated opens must start cleared.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(!app.plugin_is_opening_file);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_with_empty_plugin_content() {
    // Fresh App must have no plugin-provided content cached, neither the styled
    // spans nor the parallel plain-text store.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.plugin_content.is_empty(),
        "plugin_content must start empty"
    );
    assert!(
        app.plugin_content_text.is_empty(),
        "plugin_content_text must start empty"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_hides_dotfiles_by_default() {
    let root = temp_dir();
    fs::write(root.join("visible.txt"), "x\n").unwrap();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let app = new_app(&root, Config::default());

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&"visible.txt"), "got {names:?}");
    assert!(
        !names.contains(&".hidden"),
        "dotfile must be hidden; got {names:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_show_hidden_includes_dotfiles() {
    let root = temp_dir();
    fs::write(root.join(".hidden"), "y\n").unwrap();

    let cfg = Config {
        show_hidden: true,
        ..Config::default()
    };
    let app = new_app(&root, cfg);

    let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
    assert!(names.contains(&".hidden"), "got {names:?}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_registers_syntax_plugins_in_manager_for_palette() {
    // init.rs hands *all* plugin entries (including syntax-kind) to the
    // PluginManager so they surface in the plugin palette; the bundled
    // terraform syntax plugin is seeded into the config by default.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    let entries = app.plugin_manager.plugin_entries();
    assert!(
        entries
            .iter()
            .any(|(_, _, kind)| *kind == crate::plugin::PluginKind::Syntax),
        "a syntax plugin must be registered in the manager so it appears in the \
         plugin palette; got {entries:?}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_bundled_plugins_appear_in_config_plugins_map() {
    // Regression: bundled/manifest plugins were seeded into `cfg.plugins` only
    // *after* `saved_config = cfg.clone()`, so `self.config.plugins` was empty
    // and `toggle_plugin_picker_selection` could never persist the enabled flag.
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        !app.config.plugins.is_empty(),
        "bundled plugins must appear in config.plugins; got empty map"
    );
    // At least one bundled entry should be present (e.g. the markdown plugin).
    let bundled: Vec<String> = crate::plugin::bundled_plugin_entries()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    for name in &bundled {
        assert!(
            app.config.plugins.contains_key(name),
            "bundled plugin {name} must appear in config.plugins"
        );
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_bundled_plugin_toggle_flips_enabled_flag() {
    // Toggling a bundled plugin's enabled flag via config.plugins.get_mut
    // must succeed because the entry is present in self.config.plugins.
    let root = temp_dir();
    let mut app = new_app(&root, Config::default());
    let bundled: Vec<String> = crate::plugin::bundled_plugin_entries()
        .into_iter()
        .map(|(n, _)| n)
        .collect();
    let name = bundled.first().expect("at least one bundled plugin");
    let orig = app
        .config
        .plugins
        .get(name)
        .map(|e| e.enabled)
        .unwrap_or(false);
    if let Some(entry) = app.config.plugins.get_mut(name) {
        entry.enabled = !orig;
    }
    let flipped = app
        .config
        .plugins
        .get(name)
        .map(|e| e.enabled)
        .unwrap_or(orig);
    assert_ne!(
        orig, flipped,
        "toggling bundled plugin {name}: enabled should have flipped from {orig}"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_preserves_root_path() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.root, root);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_starts_in_normal_mode() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert!(!app.git_mode, "App::new must always start in normal mode");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_session_git_mode_ignored() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    // Manually write a session file with old-format git_mode: true.
    let key = root.to_string_lossy();
    let old = format!(
        r#"{{"version":1,"sessions":{{"{}":{{"expanded":[],"current_file":null,"content_scroll":0,"active_line":0,"git_mode":true}}}}}}"#,
        key
    );
    if let Some(p) = crate::session::sessions_path() {
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, &old).unwrap();
    }
    let app = new_app(&root, Config::default());
    assert!(
        !app.git_mode,
        "must start in normal mode even when session has git_mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_viewing_revision_starts_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.viewing_revision.is_none(),
        "App::new must initialize viewing_revision to None"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_git_seq_starts_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(app.git_seq, 0, "git_seq must be zero on construction");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_git_show_flags_reflect_config() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        git_show_untracked: false,
        git_show_ignored: true,
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert!(
        !app.git_show_untracked,
        "git_show_untracked must come from config"
    );
    assert!(
        app.git_show_ignored,
        "git_show_ignored must come from config"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_last_breadcrumb_click_is_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.last_breadcrumb_click.is_none(),
        "last_breadcrumb_click must be None on construction"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_highlight_cache_starts_empty() {
    let root = temp_dir();
    fs::write(root.join("a.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert!(
        app.content_highlight_cache.borrow().is_none(),
        "fresh App must have no cached highlights"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_defaults_to_all() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let app = new_app(&root, Config::default());
    assert_eq!(app.diff_mode, DiffMode::All);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_honours_config_staged() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        diff_mode: "staged".to_string(),
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::Staged);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_honours_config_unstaged() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        diff_mode: "unstaged".to_string(),
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::Unstaged);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_diff_mode_invalid_falls_back_to_all() {
    let root = temp_dir();
    fs::write(root.join("f.txt"), "x\n").unwrap();
    let cfg = Config {
        diff_mode: "invalid".to_string(),
        ..Config::default()
    };
    let app = new_app(&root, cfg);
    assert_eq!(app.diff_mode, DiffMode::All);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_command_usage_starts_empty() {
    let root = temp_dir();
    // Point at a fresh temp dir so no on-disk usage data is loaded.
    let state_dir = temp_dir();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);
    let app = new_app(&root, Config::default());
    std::env::remove_var("MANTIS_STATE_DIR");
    assert!(
        app.command_usage.last_used().is_none(),
        "command_usage.last_used must be None when state dir is empty"
    );
    assert!(
        app.command_usage.top_used(1).is_empty(),
        "command_usage must have no recorded commands when state dir is empty"
    );
    fs::remove_dir_all(&root).ok();
    fs::remove_dir_all(&state_dir).ok();
}

#[test]
fn blame_col_width_initialises_to_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(
        app.blame_col_width, 0,
        "blame_col_width must be zero until a render populates it"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_content_active_path_initialises_to_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.plugin_content_active_path.is_none(),
        "plugin_content_active_path must be None at construction so the first set_content is treated as first-render"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_tree_revision_starts_at_zero() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert_eq!(
        app.tree_revision, 0,
        "tree_revision must be 0 at construction"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_tree_visible_indices_starts_none() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.tree_visible_indices.is_none(),
        "tree_visible_indices must be None when no filter is active"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn app_new_cursor_positions_starts_empty() {
    let root = temp_dir();
    let app = new_app(&root, Config::default());
    assert!(
        app.cursor_positions.is_empty(),
        "cursor_positions must start empty"
    );
    fs::remove_dir_all(&root).ok();
}
