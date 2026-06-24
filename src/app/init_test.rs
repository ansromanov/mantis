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
