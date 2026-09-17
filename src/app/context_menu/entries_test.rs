use super::*;

#[test]
fn markdown_fence_grows_past_backtick_runs_in_file_content() {
    assert_eq!(markdown_code_fence("line with ``` inside"), "````");
}

#[test]
fn action_builder_preserves_id_and_label() {
    let entry = action(ContextActionId::CopyPath, "Copy absolute path");

    assert!(matches!(
        entry,
        ContextMenuEntry::Action {
            id: ContextActionId::CopyPath,
            label
        } if label == "Copy absolute path"
    ));
}

#[test]
fn entry_action_id_returns_none_for_plugin_action() {
    let entry = ContextMenuEntry::PluginAction {
        plugin: "demo".to_string(),
        id: "act".to_string(),
        label: "Act".to_string(),
    };
    assert_eq!(entry_action_id(&entry), None);
}

#[test]
fn append_plugin_entries_filters_and_groups() {
    use crate::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(dir.path().to_path_buf(), Config::default(), None, None).unwrap();
    app.plugin_manager.register_context_items(
        "demo",
        vec![
            PluginContextItem {
                id: "act_content".into(),
                label: "Content Action".into(),
                target: ContextItemTargetKind::Content,
                extensions: None,
                category: None,
                weight: Some(10),
            },
            PluginContextItem {
                id: "act_cat".into(),
                label: "Categorized Action".into(),
                target: ContextItemTargetKind::Content,
                extensions: None,
                category: Some("Tools".into()),
                weight: Some(5),
            },
            PluginContextItem {
                id: "act_tree".into(),
                label: "Tree Action".into(),
                target: ContextItemTargetKind::TreeFile,
                extensions: Some(vec!["rs".into()]),
                category: None,
                weight: None,
            },
        ],
    );

    let mut entries = vec![action(ContextActionId::Open, "Open")];
    append_plugin_entries(&app, &mut entries, ContextItemTargetKind::Content, None);

    // Should have separator, submenu "Tools", and top-level action "Content Action"
    assert_eq!(entries.len(), 4);
    assert!(matches!(entries[0], ContextMenuEntry::Action { .. }));
    assert!(matches!(entries[1], ContextMenuEntry::Separator));
    assert!(matches!(entries[2], ContextMenuEntry::Submenu { ref label, .. } if label == "Tools"));
    assert!(
        matches!(entries[3], ContextMenuEntry::PluginAction { ref id, .. } if id == "act_content")
    );

    // Extension matching on TreeFile
    let mut tree_entries = Vec::new();
    append_plugin_entries(
        &app,
        &mut tree_entries,
        ContextItemTargetKind::TreeFile,
        Some(std::path::Path::new("main.rs")),
    );
    assert_eq!(tree_entries.len(), 1);
    assert!(
        matches!(tree_entries[0], ContextMenuEntry::PluginAction { ref id, .. } if id == "act_tree")
    );

    let mut tree_no_match = Vec::new();
    append_plugin_entries(
        &app,
        &mut tree_no_match,
        ContextItemTargetKind::TreeFile,
        Some(std::path::Path::new("main.py")),
    );
    assert!(tree_no_match.is_empty());
}

#[test]
fn statusbar_entries_includes_git_mode_toggle_when_git_segment() {
    use crate::config::Config;
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(dir.path().to_path_buf(), Config::default(), None, None).unwrap();
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".into()),
        ahead: 0,
        behind: 0,
        total_changed: 1,
        staged: 0,
        untracked: 0,
    });

    let entries = statusbar_entries(&app, "main");
    assert!(entries.iter().any(|e| matches!(e, ContextMenuEntry::Action { id: ContextActionId::DiffVsHead, label } if label.contains("Toggle git mode"))));

    app.git_mode = true;
    let entries_in_git_mode = statusbar_entries(&app, "main");
    assert!(entries_in_git_mode.iter().any(|e| matches!(e, ContextMenuEntry::Action { id: ContextActionId::DiffVsHead, label } if label.contains("Exit git mode"))));
}
