use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, ContextActionId, ContextMenuEntry, ContextMenuTarget};
use crate::config::Config;
use crate::selection::TextSelection;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_cm_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
    fs::create_dir_all(dir.join("sub")).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, crossterm::event::KeyModifiers::empty())
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: crossterm::event::KeyModifiers::empty(),
    }
}

fn action_labels(menu: &crate::app::ContextMenuState) -> Vec<String> {
    menu.entries
        .iter()
        .filter_map(|e| match e {
            ContextMenuEntry::Action { label, .. } => Some(label.clone()),
            ContextMenuEntry::Separator => None,
        })
        .collect()
}

fn entry_id(menu: &crate::app::ContextMenuState, index: usize) -> Option<ContextActionId> {
    match menu.entries.get(index)? {
        ContextMenuEntry::Action { id, .. } => Some(*id),
        ContextMenuEntry::Separator => None,
    }
}

#[test]
fn open_tree_context_menu_on_file_stores_tree_target_and_focuses_tree() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .expect("a.txt must be in the tree");
    app.tree_selected = 0;
    app.focus = crate::app::Focus::Content;

    app.open_tree_context_menu(file_idx, (10, 10));

    let menu = app.context_menu.expect("menu must open");
    assert!(matches!(
        &menu.target,
        ContextMenuTarget::Tree { path, index } if *path == root.join("a.txt") && *index == file_idx
    ));
    assert_eq!(
        app.tree_selected, file_idx,
        "clicked row must become selected"
    );
    assert_eq!(app.focus, crate::app::Focus::Tree);
    let labels = action_labels(&menu);
    assert!(labels.iter().any(|l| l == "Open in editor"));
    assert!(labels.iter().any(|l| l == "Open with default app"));
    assert!(
        !labels.iter().any(|l| l == "Expand" || l == "Collapse"),
        "file menu must not offer directory-only expand/collapse"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_tree_context_menu_on_collapsed_dir_offers_expand() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .expect("sub dir must be in the tree");

    app.open_tree_context_menu(dir_idx, (10, 10));

    let menu = app.context_menu.expect("menu must open");
    let labels = action_labels(&menu);
    assert!(
        labels.iter().any(|l| l == "Expand"),
        "collapsed dir menu must offer Expand"
    );
    assert!(
        !labels.iter().any(|l| l == "Collapse"),
        "collapsed dir menu must not offer Collapse"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_tree_context_menu_on_expanded_dir_offers_collapse() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .expect("sub dir must be in the tree");
    app.expanded.insert(root.join("sub"));

    app.open_tree_context_menu(dir_idx, (10, 10));

    let menu = app.context_menu.expect("menu must open");
    let labels = action_labels(&menu);
    assert!(
        labels.iter().any(|l| l == "Collapse"),
        "expanded dir menu must offer Collapse"
    );
    assert!(
        !labels.iter().any(|l| l == "Expand"),
        "expanded dir menu must not offer Expand"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_content_context_menu_without_file_omits_file_actions() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.current_file = None;

    app.open_content_context_menu((20, 5));

    let menu = app.context_menu.expect("menu must open");
    assert!(matches!(menu.target, ContextMenuTarget::Content));
    let labels = action_labels(&menu);
    assert!(labels.iter().any(|l| l == "Copy line"));
    assert!(labels.iter().any(|l| l == "Word wrap: off"));
    assert!(
        !labels.iter().any(|l| l == "Reveal in tree"),
        "content menu without an open file must omit file actions"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_content_context_menu_on_markdown_with_plugin_offers_raw_toggle() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let md = root.join("doc.md");
    fs::write(&md, "# Hi").unwrap();
    app.open_file(&md);
    app.plugin_manager
        .plugins
        .push(crate::plugin::Plugin::new("markdown".to_string(), vec![]));

    app.open_content_context_menu((20, 5));

    let menu = app.context_menu.expect("menu must open");
    let labels = action_labels(&menu);
    assert!(
        labels.iter().any(|l| l == "Rendered markdown: on"),
        "active markdown plugin on a .md file must expose the raw/rendered toggle"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn content_menu_offers_copy_selection_only_with_live_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));

    app.open_content_context_menu((20, 5));
    let labels = action_labels(app.context_menu.as_ref().unwrap());
    assert!(
        !labels.iter().any(|l| l == "Copy selection"),
        "no selection open menu must omit Copy selection"
    );

    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 5),
    });
    app.open_content_context_menu((20, 5));
    let labels = action_labels(app.context_menu.as_ref().unwrap());
    assert!(
        labels.iter().any(|l| l == "Copy selection"),
        "menu with a live selection must offer Copy selection"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_esc_closes_menu() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_tree_context_menu(0, (10, 10));
    assert!(app.context_menu.is_some());

    app.handle_context_menu_key(key(KeyCode::Esc));

    assert!(
        app.context_menu.is_none(),
        "Esc must close the context menu"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_down_navigation_skips_separators_and_clamps() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_tree_context_menu(0, (10, 10));

    let ids: Vec<ContextActionId> = app
        .context_menu
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .filter_map(entry_action_id)
        .collect();
    let last_action = app
        .context_menu
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .rposition(|e| matches!(e, ContextMenuEntry::Action { .. }))
        .expect("menu must have actions");
    assert!(ids.len() >= 6, "menu must have several selectable actions");

    for _ in 0..(ids.len() + 3) {
        app.handle_context_menu_key(key(KeyCode::Down));
    }
    let selected = app.context_menu.as_ref().unwrap().selected;
    assert_eq!(
        selected, last_action,
        "repeated Down must clamp at the last action, not past the end"
    );
    assert!(
        matches!(
            app.context_menu.as_ref().unwrap().entries.get(selected),
            Some(ContextMenuEntry::Action { .. })
        ),
        "selection must always sit on an action row, never a separator"
    );

    app.handle_context_menu_key(key(KeyCode::Char('k')));
    let selected = app.context_menu.as_ref().unwrap().selected;
    assert!(matches!(
        app.context_menu.as_ref().unwrap().entries.get(selected),
        Some(ContextMenuEntry::Action { .. })
    ));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_enter_runs_default_open_action_on_directory() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .unwrap();
    let sub = root.join("sub");
    assert!(!app.expanded.contains(&sub));

    app.open_tree_context_menu(dir_idx, (10, 10));
    assert_eq!(
        entry_id(app.context_menu.as_ref().unwrap(), 0),
        Some(ContextActionId::Open)
    );
    app.handle_context_menu_key(key(KeyCode::Enter));

    assert!(
        app.expanded.contains(&sub),
        "Enter on a collapsed dir's menu must expand it"
    );
    assert!(
        app.context_menu.is_none(),
        "activating an action must close the menu"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_enter_on_expand_entry_expands_directory() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == root.join("sub"))
        .unwrap();
    let sub = root.join("sub");

    app.open_tree_context_menu(dir_idx, (10, 10));
    // Directory menu: 0 Open, sep, 2 CopyPath, 3 CopyRelative, sep, 5 Reveal,
    // 6 Expand. Four downs skip the separators to land on Expand.
    for _ in 0..4 {
        app.handle_context_menu_key(key(KeyCode::Down));
    }
    assert_eq!(
        entry_id(
            app.context_menu.as_ref().unwrap(),
            app.context_menu.as_ref().unwrap().selected
        ),
        Some(ContextActionId::ExpandDir)
    );
    app.handle_context_menu_key(key(KeyCode::Enter));

    assert!(
        app.expanded.contains(&sub),
        "Expand action must expand the dir"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_enter_on_collapse_entry_collapses_directory() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let sub = root.join("sub");
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == sub)
        .unwrap();
    app.expanded.insert(sub.clone());

    app.open_tree_context_menu(dir_idx, (10, 10));
    for _ in 0..4 {
        app.handle_context_menu_key(key(KeyCode::Down));
    }
    assert_eq!(
        entry_id(
            app.context_menu.as_ref().unwrap(),
            app.context_menu.as_ref().unwrap().selected
        ),
        Some(ContextActionId::CollapseDir)
    );
    app.handle_context_menu_key(key(KeyCode::Enter));

    assert!(
        !app.expanded.contains(&sub),
        "Collapse action must collapse the dir"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_copy_path_copies_absolute_path() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .unwrap();
    app.open_tree_context_menu(file_idx, (10, 10));

    app.execute_context_action(ContextActionId::CopyPath);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(root.join("a.txt").to_str().unwrap()),
        "CopyPath must copy the target's absolute path"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_copy_relative_path_copies_path_relative_to_root() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let file_idx = app
        .nodes
        .iter()
        .position(|n| n.path == root.join("a.txt"))
        .unwrap();
    app.open_tree_context_menu(file_idx, (10, 10));

    app.execute_context_action(ContextActionId::CopyRelativePath);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("a.txt"),
        "CopyRelativePath must strip the viewer root"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_content_menu_copy_path_uses_current_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.open_content_context_menu((20, 5));

    app.execute_context_action(ContextActionId::CopyPath);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some(root.join("a.txt").to_str().unwrap())
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_content_menu_copy_line_copies_active_line() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.active_line = 1;
    app.open_content_context_menu((20, 5));

    app.execute_context_action(ContextActionId::CopyLine);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line2"),
        "CopyLine must copy the active line"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_content_menu_copy_selection_copies_selected_text() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.content_area = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };
    app.content_scroll = 0;
    app.selection = Some(TextSelection {
        anchor: (0, 0),
        active: (0, 5),
    });

    app.execute_context_action(ContextActionId::CopySelection);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line1"),
        "CopySelection must copy the live selection"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_copy_file_action_ignored_without_selection() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.selection = None;

    app.execute_context_action(ContextActionId::CopySelection);

    assert!(
        app.clipboard_capture.is_empty(),
        "CopySelection without a selection must be a no-op"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_content_menu_copy_file_copies_whole_file() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));

    app.execute_context_action(ContextActionId::CopyFile);

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line1\nline2"),
        "CopyFile must copy the whole file content"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_toggle_word_wrap_flips_state_and_persists() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.word_wrap = false;

    app.execute_context_action(ContextActionId::ToggleWordWrap);

    assert!(app.word_wrap, "ToggleWordWrap must flip word_wrap on");
    assert!(
        app.config.content.word_wrap,
        "ToggleWordWrap must persist to config"
    );

    app.execute_context_action(ContextActionId::ToggleWordWrap);
    assert!(
        !app.word_wrap,
        "ToggleWordWrap must flip word_wrap off again"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_toggle_raw_markdown_without_plugin_shows_status() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));

    app.execute_context_action(ContextActionId::ToggleRawMarkdown);

    assert!(
        app.status_message
            .as_ref()
            .is_some_and(|s| s.text.contains("not available")),
        "raw markdown toggle without the plugin must surface a status message"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_content_menu_reveal_in_tree_searches_node() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.focus = crate::app::Focus::Content;

    app.execute_context_action(ContextActionId::RevealInTree);

    assert_eq!(app.focus, crate::app::Focus::Tree);
    assert!(
        app.nodes
            .get(app.tree_selected)
            .is_some_and(|n| n.path == root.join("a.txt")),
        "RevealInTree must select the open file's node"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_menu_mouse_click_on_item_activates() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_file(&root.join("a.txt"));
    app.open_content_context_menu((20, 5));
    // Popup rows start one cell below the top border: row 10 (y=9+1) is entry 0.
    app.context_menu_area = Rect {
        x: 10,
        y: 9,
        width: 30,
        height: 16,
    };

    // Entry 0 with current_file set is CopyLine → copies the active line.
    app.handle_context_menu_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 11, 10));

    assert_eq!(
        app.clipboard_capture.last().map(String::as_str),
        Some("line1"),
        "left click on the first action row must run it"
    );
    assert!(
        app.context_menu.is_none(),
        "activating via click must close the menu"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_menu_mouse_left_outside_closes() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_tree_context_menu(0, (10, 10));
    app.context_menu_area = Rect {
        x: 10,
        y: 10,
        width: 30,
        height: 12,
    };

    app.handle_context_menu_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 2, 2));

    assert!(
        app.context_menu.is_none(),
        "left click outside the popup must dismiss the menu"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_menu_mouse_right_closes_without_reopening() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_tree_context_menu(0, (10, 10));

    app.handle_context_menu_mouse(mouse(MouseEventKind::Down(MouseButton::Right), 50, 5));

    assert!(
        app.context_menu.is_none(),
        "right click while the menu is open must dismiss it"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_menu_mouse_wheel_navigates() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.open_tree_context_menu(0, (10, 10));

    app.handle_context_menu_mouse(mouse(MouseEventKind::ScrollDown, 50, 5));
    let after_down = app.context_menu.as_ref().unwrap().selected;
    assert!(
        after_down == 2,
        "wheel-down must move to the next action, skipping separators (got {after_down})"
    );

    app.handle_context_menu_mouse(mouse(MouseEventKind::ScrollUp, 50, 5));
    assert_eq!(
        app.context_menu.as_ref().unwrap().selected,
        0,
        "wheel-up must move back to the previous action"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn context_expand_dir_action_uses_target_path_after_rebuild() {
    let root = temp_tree();
    let mut app = app_for(&root);
    let sub = root.join("sub");
    let dir_idx = app
        .nodes
        .iter()
        .position(|n| n.is_dir && n.path == sub && n.depth == 0)
        .unwrap();
    app.open_tree_context_menu(dir_idx, (10, 10));
    // Move the selection off the target: the action should still act on the
    // stored path, not on whatever is currently selected.
    app.tree_selected = dir_idx + 1;

    app.execute_context_action(ContextActionId::ExpandDir);

    assert!(
        app.expanded.contains(&sub),
        "ExpandDir must expand the right-clicked path even if selection moved"
    );
    fs::remove_dir_all(&root).ok();
}
