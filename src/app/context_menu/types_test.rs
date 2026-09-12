use super::*;

#[test]
fn submenu_selection_is_scoped_to_visible_level() {
    let mut state = ContextMenuState {
        entries: vec![ContextMenuEntry::Action {
            id: ContextActionId::Open,
            label: "Open".to_string(),
        }],
        selected: 0,
        anchor: (0, 0),
        target: ContextMenuTarget::Content,
        submenu_stack: Vec::new(),
    };

    state.push_submenu(vec![ContextMenuEntry::Action {
        id: ContextActionId::CopyPath,
        label: "Copy path".to_string(),
    }]);
    state.set_visible_selected(0);

    assert_eq!(state.visible_entries().len(), 1);
    assert_eq!(state.visible_selected(), 0);
    assert_eq!(state.selected, 0);
}
