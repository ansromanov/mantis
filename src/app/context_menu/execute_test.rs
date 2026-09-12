use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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
