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
