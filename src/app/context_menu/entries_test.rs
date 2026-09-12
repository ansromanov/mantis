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
