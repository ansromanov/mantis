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

#[test]
fn max_label_len_accounts_for_plugin_action() {
    let state = ContextMenuState {
        entries: vec![
            ContextMenuEntry::Action {
                id: ContextActionId::Open,
                label: "Open".to_string(), // len 4
            },
            ContextMenuEntry::PluginAction {
                plugin: "demo".to_string(),
                id: "run".to_string(),
                label: "Run Plugin Action".to_string(), // len 17
            },
            ContextMenuEntry::Separator,
            ContextMenuEntry::Submenu {
                label: "Sub".to_string(), // len 3 + 3 = 6
                entries: Vec::new(),
            },
        ],
        selected: 0,
        anchor: (0, 0),
        target: ContextMenuTarget::Content,
        submenu_stack: Vec::new(),
    };
    assert_eq!(state.max_label_len(), 17);
}
