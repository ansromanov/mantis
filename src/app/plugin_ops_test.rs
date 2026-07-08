//! Tests for `App` plugin lifecycle methods (see `plugin_ops.rs`).

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::App;
use crate::config::Config;
use crate::plugin::{PluginEntry, PluginKind};
use crate::search::PluginPicker;

fn temp_dir() -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_plugin_ops_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn new_app_with_syntax_plugin(root: &std::path::Path, name: &str, enabled: bool) -> App {
    let mut cfg = Config::default();
    cfg.plugins.insert(
        name.to_string(),
        PluginEntry {
            path: PathBuf::new(),
            enabled,
            kind: PluginKind::Syntax,
            extensions: vec![],
            syntax_file: None,
            events: vec![],
        },
    );
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

#[test]
fn toggle_plugin_picker_selection_enables_disabled_syntax_plugin() {
    let root = temp_dir();
    let mut app = new_app_with_syntax_plugin(&root, "my-syntax", false);
    app.plugin_picker = Some(PluginPicker::new(vec![(
        "my-syntax".to_string(),
        false,
        PluginKind::Syntax,
        None,
    )]));

    app.toggle_plugin_picker_selection();

    assert!(
        app.config.plugins.get("my-syntax").unwrap().enabled,
        "toggling a disabled syntax plugin must flip its enabled flag on"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_plugin_picker_selection_disables_enabled_syntax_plugin() {
    let root = temp_dir();
    let mut app = new_app_with_syntax_plugin(&root, "my-syntax", true);
    app.plugin_picker = Some(PluginPicker::new(vec![(
        "my-syntax".to_string(),
        true,
        PluginKind::Syntax,
        None,
    )]));

    app.toggle_plugin_picker_selection();

    assert!(
        !app.config.plugins.get("my-syntax").unwrap().enabled,
        "toggling an enabled syntax plugin must flip its enabled flag off"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_plugin_picker_selection_refreshes_picker_entries() {
    let root = temp_dir();
    let mut app = new_app_with_syntax_plugin(&root, "my-syntax", false);
    app.plugin_picker = Some(PluginPicker::new(vec![(
        "my-syntax".to_string(),
        false,
        PluginKind::Syntax,
        None,
    )]));

    app.toggle_plugin_picker_selection();

    let picker = app.plugin_picker.expect("picker must survive the toggle");
    assert_eq!(
        picker.entries,
        app.plugin_manager.plugin_entries(),
        "picker entries must be refreshed from the manager after a toggle, \
         so the checkbox reflects the new state immediately"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn toggle_plugin_picker_selection_noop_when_no_picker() {
    let root = temp_dir();
    let mut app = new_app_with_syntax_plugin(&root, "my-syntax", false);
    app.plugin_picker = None;

    app.toggle_plugin_picker_selection();

    assert!(
        !app.config.plugins.get("my-syntax").unwrap().enabled,
        "with no picker open there is no selection to toggle"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn plugin_ops_telemetry_check() {
    let root = temp_dir();
    let app = new_app_with_syntax_plugin(&root, "my-syntax", false);
    assert!(!app.telemetry.is_enabled());
    fs::remove_dir_all(&root).ok();
}
