use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::{ContextMenuState, ContextMenuTarget};
use crate::config::Config;
use crate::search::SearchMode;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mantis_execute_test_{}_{n}", std::process::id()));
    fs::create_dir_all(dir.join("sub")).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn find_in_folder_opens_content_search_from_that_directory() {
    let root = temp_tree();
    let mut app = App::new(root.clone(), Config::default(), None, None).unwrap();
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .unwrap();
    app.open_tree_context_menu(dir_idx, (10, 10));

    app.execute_context_action(ContextActionId::FindInFolder);

    let search = app.search.as_ref().expect("scoped search opens");
    assert_eq!(search.mode, SearchMode::Content);
    assert!(app.context_menu.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn execute_plugin_context_action_closes_menu_and_dispatches() {
    let root = temp_tree();
    let mut app = App::new(root.clone(), Config::default(), None, None).unwrap();
    app.context_menu = Some(ContextMenuState {
        entries: vec![ContextMenuEntry::PluginAction {
            plugin: "demo".to_string(),
            id: "my_action".to_string(),
            label: "My Action".to_string(),
        }],
        selected: 0,
        anchor: (10, 10),
        target: ContextMenuTarget::Content,
        submenu_stack: Vec::new(),
    });

    app.activate_context_selection();
    assert!(
        app.context_menu.is_none(),
        "menu should close after activating plugin action"
    );
    fs::remove_dir_all(&root).ok();
}
