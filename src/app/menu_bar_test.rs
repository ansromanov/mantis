use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::config::{bind, Config};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn app() -> (App, PathBuf) {
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("mantis-menu-bar-{}-{id}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let app = App::new(root.clone(), Config::default(), None, None).unwrap();
    (app, root)
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

#[test]
fn each_palette_action_has_exactly_one_menu_placement() {
    let actions = crate::actions::ACTIONS;
    for action in actions.iter().filter(|action| action.palette.is_some()) {
        let placements = MENUS
            .iter()
            .filter(|menu| action.menu.is_some_and(|(name, _)| name == **menu))
            .count();
        assert_eq!(placements, 1, "{} must occur in one menu", action.id);
    }
    assert!(actions
        .iter()
        .filter(|action| action.palette.is_some())
        .all(|action| action.menu.is_some()));
}

#[test]
fn f10_opens_menu_when_persistent_row_is_disabled() {
    let (mut app, root) = app();
    app.handle_key(key(KeyCode::F(10)));
    assert!(app.menu_bar_state.is_some());
    assert!(!app.config.ui.menu_bar);
    app.handle_key(key(KeyCode::Esc));
    assert!(app.menu_bar_state.is_none());
    app.handle_key(KeyEvent::new(KeyCode::Null, KeyModifiers::ALT));
    assert!(app.menu_bar_state.is_some());
    drop(app);
    fs::remove_dir_all(root).ok();
}

#[test]
fn menu_keys_change_groups_and_dispatch_the_selected_action() {
    let (mut app, root) = app();
    app.open_menu_bar();
    app.handle_menu_bar_key(key(KeyCode::Right));
    assert_eq!(app.menu_bar_state.unwrap().menu_index, 1);
    app.open_menu_bar();
    app.handle_menu_bar_key(key(KeyCode::Enter));
    assert!(app.show_help);
    assert!(app.menu_bar_state.is_none());
    drop(app);
    fs::remove_dir_all(root).ok();
}

#[test]
fn menu_binding_labels_follow_the_live_keymap() {
    let (mut app, root) = app();
    app.keys.quit = bind(&["z"]);
    let general = menu_actions(&app, "General");
    let quit = general
        .iter()
        .find(|item| item.id == "quit")
        .expect("quit action should appear in General");
    assert_eq!(quit.binding, "z");
    assert_eq!(
        menu_actions(&app, "View")
            .iter()
            .find(|item| item.id == "fold_all")
            .unwrap()
            .binding,
        "-"
    );
    drop(app);
    fs::remove_dir_all(root).ok();
}

#[test]
fn inapplicable_menu_action_is_dimmed_and_refused() {
    let (mut app, root) = app();
    let git = menu_actions(&app, "Git");
    let index = git
        .iter()
        .position(|item| item.id == "file_history")
        .expect("file history should appear in Git");
    assert!(!git[index].applicable);
    app.menu_bar_state = Some(MenuBarState {
        menu_index: MENUS.iter().position(|name| *name == "Git").unwrap(),
        selected: index,
    });
    app.handle_menu_bar_key(key(KeyCode::Enter));
    assert!(app.menu_bar_state.is_none());
    assert!(app
        .status_message
        .as_ref()
        .is_some_and(|status| status.text.contains("no file is open")));
    drop(app);
    fs::remove_dir_all(root).ok();
}

#[test]
fn clicking_a_menu_row_then_an_action_dispatches_it() {
    let (mut app, root) = app();
    app.menu_bar_area = Rect::new(0, 0, 80, 1);
    app.menu_dropdown_area = Rect::new(0, 1, 40, 10);
    app.handle_menu_bar_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 0,
        modifiers: KeyModifiers::empty(),
    });
    assert!(app.menu_bar_state.is_some());
    app.handle_menu_bar_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 2,
        modifiers: KeyModifiers::empty(),
    });
    assert!(app.show_help);
    assert!(app.menu_bar_state.is_none());
    drop(app);
    fs::remove_dir_all(root).ok();
}
