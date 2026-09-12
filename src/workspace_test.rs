use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::config::Config;

fn temp_dir(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "mantis_workspace_{name}_{}_{n}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::empty()))
}

fn ctrl_key(c: char) -> Event {
    Event::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL))
}

fn ctrl_enter() -> Event {
    Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL))
}

fn worktree_item(path: &std::path::Path) -> crate::git::WorktreeItem {
    crate::git::WorktreeItem {
        worktree: crate::git::Worktree {
            path: path.to_path_buf(),
            head: "abc".into(),
            branch: Some("agent".into()),
            locked: false,
            prunable: false,
        },
        changed: 0,
    }
}

/// Points `MANTIS_STATE_DIR` at an isolated dir for the duration of the test.
struct IsolatedState {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl IsolatedState {
    fn new(dir: &std::path::Path) -> Self {
        let lock = crate::session::STATE_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let state = dir.join("state");
        fs::create_dir_all(&state).unwrap();
        std::env::set_var("MANTIS_STATE_DIR", &state);
        Self { _lock: lock }
    }
}

impl Drop for IsolatedState {
    fn drop(&mut self) {
        std::env::remove_var("MANTIS_STATE_DIR");
    }
}

#[test]
fn new_clamps_active_index_to_valid_range() {
    let dir = temp_dir("clamp");
    let tabs = Tabs::new(vec![app_for(&dir)], 5);
    assert_eq!(tabs.active, 0);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn open_tab_pushes_and_activates_the_new_tab() {
    let a = temp_dir("open_a");
    let b = temp_dir("open_b");
    let mut tabs = Tabs::new(vec![app_for(&a)], 0);
    tabs.open_tab(b.clone()).unwrap();
    assert_eq!(tabs.apps.len(), 2);
    assert_eq!(tabs.active, 1);
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn open_tab_activates_an_existing_canonical_root_without_duplicating_it() {
    let a = temp_dir("dedupe_a");
    let b = temp_dir("dedupe_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let aliased_b = b.join(".");

    tabs.open_tab(aliased_b).unwrap();

    assert_eq!(tabs.apps.len(), 2);
    assert_eq!(tabs.active, 1);
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn ctrl_enter_opens_or_activates_the_selected_worktree_tab() {
    let a = temp_dir("action_a");
    let b = temp_dir("action_b");
    let c = temp_dir("action_c");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b), app_for(&c)], 2);
    tabs.active_app_mut().worktree_picker = Some(crate::search::WorktreePicker::for_test(vec![
        worktree_item(&b),
    ]));

    tabs.dispatch_event(ctrl_enter());

    assert_eq!(
        tabs.apps.len(),
        3,
        "an open worktree must not be duplicated"
    );
    assert_eq!(
        tabs.active, 1,
        "the existing worktree tab must be activated"
    );
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
    fs::remove_dir_all(&c).ok();
}

#[test]
fn opening_a_root_uses_its_session_without_changing_the_active_tab() {
    let a = temp_dir("session_a");
    let b = temp_dir("session_b");
    fs::write(a.join("current.txt"), "alpha\nbeta\n").unwrap();
    fs::write(b.join("current.txt"), "one\ntwo\nthree\n").unwrap();
    let _state = IsolatedState::new(&a);
    let b_file = b.join("current.txt");
    crate::session::save(
        &b,
        &crate::session::SessionState {
            current_file: Some(b_file.clone()),
            content_scroll: 1,
            active_line: 2,
            ..Default::default()
        },
    );
    let mut current = app_for(&a);
    current.active_line = 1;
    current.content_scroll = 0;
    let mut tabs = Tabs::new(vec![current], 0);

    tabs.open_tab(b.clone()).unwrap();

    assert_eq!(tabs.apps.len(), 2);
    assert_eq!(tabs.active_app().root, b);
    assert_eq!(
        tabs.active_app().current_file.as_deref(),
        Some(b_file.as_path())
    );
    assert_eq!(tabs.active_app().active_line, 2);
    assert_eq!(tabs.apps[0].active_line, 1);
    assert_eq!(tabs.apps[0].content_scroll, 0);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn close_active_tab_refuses_to_drop_the_last_tab() {
    let dir = temp_dir("close_last");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    tabs.close_active_tab();
    assert_eq!(tabs.apps.len(), 1, "the only tab must stay open");
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn close_active_tab_removes_it_and_clamps_active() {
    let a = temp_dir("close_a");
    let b = temp_dir("close_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 1);
    tabs.close_active_tab();
    assert_eq!(tabs.apps.len(), 1);
    assert_eq!(tabs.active, 0);
    assert_eq!(tabs.active_app().root, a);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn next_and_prev_tab_wrap_around() {
    let a = temp_dir("wrap_a");
    let b = temp_dir("wrap_b");
    let c = temp_dir("wrap_c");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b), app_for(&c)], 2);
    tabs.next_tab();
    assert_eq!(
        tabs.active, 0,
        "next_tab must wrap from the last to the first"
    );
    tabs.prev_tab();
    assert_eq!(
        tabs.active, 2,
        "prev_tab must wrap from the first to the last"
    );
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
    fs::remove_dir_all(&c).ok();
}

#[test]
fn next_and_prev_tab_are_no_ops_with_a_single_tab() {
    let dir = temp_dir("single");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    tabs.next_tab();
    tabs.prev_tab();
    assert_eq!(tabs.active, 0);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn select_clamps_and_move_keeps_the_moved_tab_active() {
    let a = temp_dir("select_a");
    let b = temp_dir("select_b");
    let c = temp_dir("select_c");
    let _state = IsolatedState::new(&a);
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b), app_for(&c)], 0);
    tabs.select_tab(99);
    assert_eq!(tabs.active, 2);
    tabs.move_tab(2, 0);
    assert_eq!(tabs.active, 0);
    assert_eq!(tabs.active_app().root, c);
    tabs.move_active_tab(1);
    assert_eq!(tabs.active, 1);
    assert_eq!(tabs.active_app().root, c);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
    fs::remove_dir_all(&c).ok();
}

#[test]
fn reordered_tabs_are_written_in_manifest_order() {
    let a = temp_dir("move_persist_a");
    let b = temp_dir("move_persist_b");
    let _state = IsolatedState::new(&a);
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    tabs.move_active_tab(1);
    tabs.save_all_and_persist_workspace();
    let loaded = crate::session::load_workspace().unwrap();
    assert_eq!(loaded.roots, vec![b.clone(), a.clone()]);
    assert_eq!(loaded.active, 1);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn close_and_reopen_restores_the_last_root() {
    let a = temp_dir("reopen_a");
    let b = temp_dir("reopen_b");
    let _state = IsolatedState::new(&a);
    let remembered = b.join("remembered.txt");
    fs::write(&remembered, "remember this file").unwrap();
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 1);
    tabs.active_app_mut().open_file(&remembered);
    tabs.close_active_tab();
    assert_eq!(tabs.active_app().root, a);
    tabs.reopen_closed_tab();
    assert_eq!(tabs.active_app().root, b);
    assert_eq!(tabs.active_app().current_file.as_ref(), Some(&remembered));
    assert!(tabs.closed_tabs.is_empty());
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn save_all_and_persist_workspace_writes_the_manifest() {
    let a = temp_dir("persist_a");
    let b = temp_dir("persist_b");
    let _state = IsolatedState::new(&a);
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 1);
    tabs.save_all_and_persist_workspace();
    let loaded = crate::session::load_workspace().expect("workspace manifest must be written");
    assert_eq!(loaded.roots, vec![a.clone(), b.clone()]);
    assert_eq!(loaded.active, 1);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn ctrl_n_opens_the_new_tab_prompt() {
    let dir = temp_dir("prompt_open");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    assert!(tabs.new_tab_prompt.is_none());
    tabs.dispatch_event(ctrl_key('n'));
    assert_eq!(tabs.new_tab_prompt.as_deref(), Some(""));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn new_tab_prompt_esc_cancels() {
    let dir = temp_dir("prompt_cancel");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    tabs.new_tab_prompt = Some("some/path".to_string());
    tabs.dispatch_event(key(KeyCode::Esc));
    assert!(tabs.new_tab_prompt.is_none());
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn new_tab_prompt_types_and_backspaces() {
    let dir = temp_dir("prompt_type");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    tabs.new_tab_prompt = Some(String::new());
    tabs.dispatch_event(key(KeyCode::Char('a')));
    tabs.dispatch_event(key(KeyCode::Char('b')));
    assert_eq!(tabs.new_tab_prompt.as_deref(), Some("ab"));
    tabs.dispatch_event(key(KeyCode::Backspace));
    assert_eq!(tabs.new_tab_prompt.as_deref(), Some("a"));
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn new_tab_prompt_enter_with_valid_directory_opens_a_tab() {
    let a = temp_dir("prompt_valid_a");
    let b = temp_dir("prompt_valid_b");
    let mut tabs = Tabs::new(vec![app_for(&a)], 0);
    tabs.new_tab_prompt = Some(b.display().to_string());
    tabs.dispatch_event(key(KeyCode::Enter));
    assert!(tabs.new_tab_prompt.is_none());
    assert_eq!(tabs.apps.len(), 2);
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn new_tab_prompt_enter_with_bad_path_stays_open_with_an_error() {
    let dir = temp_dir("prompt_bad");
    let missing = dir.join("does_not_exist");
    let mut tabs = Tabs::new(vec![app_for(&dir)], 0);
    tabs.new_tab_prompt = Some(missing.display().to_string());
    tabs.dispatch_event(key(KeyCode::Enter));
    assert_eq!(tabs.apps.len(), 1, "no tab should open for a bad path");
    assert!(
        tabs.new_tab_prompt.is_some(),
        "the prompt must stay open so the user can correct it"
    );
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn ctrl_w_closes_the_active_tab() {
    let a = temp_dir("ctrlw_a");
    let b = temp_dir("ctrlw_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 1);
    tabs.dispatch_event(ctrl_key('w'));
    assert_eq!(tabs.apps.len(), 1);
    assert_eq!(tabs.active_app().root, a);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn ctrl_pagedown_and_pageup_switch_tabs() {
    let a = temp_dir("pgdn_a");
    let b = temp_dir("pgdn_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    tabs.dispatch_event(Event::Key(KeyEvent::new(
        KeyCode::PageDown,
        KeyModifiers::CONTROL,
    )));
    assert_eq!(tabs.active, 1);
    tabs.dispatch_event(Event::Key(KeyEvent::new(
        KeyCode::PageUp,
        KeyModifiers::CONTROL,
    )));
    assert_eq!(tabs.active, 0);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn ctrl_number_selects_tab_and_zero_selects_last() {
    let roots = (0..3)
        .map(|index| temp_dir(&format!("direct_{index}")))
        .collect::<Vec<_>>();
    let mut tabs = Tabs::new(roots.iter().map(|root| app_for(root)).collect(), 0);
    tabs.dispatch_event(Event::Key(KeyEvent::new(
        KeyCode::Char('3'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(tabs.active, 2);
    tabs.dispatch_event(Event::Key(KeyEvent::new(
        KeyCode::Char('0'),
        KeyModifiers::CONTROL,
    )));
    assert_eq!(tabs.active, 2);
    for root in roots {
        fs::remove_dir_all(root).ok();
    }
}

#[test]
fn dragging_a_tab_reorders_it_on_release() {
    let a = temp_dir("drag_alpha");
    let b = temp_dir("drag_beta");
    let _state = IsolatedState::new(&a);
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    tabs.strip_area = ratatui::layout::Rect::new(0, 0, 80, 1);
    let first_width = a.file_name().unwrap().to_string_lossy().len() as u16 + 4;
    let mouse = |kind, column| {
        Event::Mouse(MouseEvent {
            kind,
            column,
            row: 0,
            modifiers: KeyModifiers::empty(),
        })
    };
    tabs.dispatch_event(mouse(MouseEventKind::Down(MouseButton::Left), 1));
    tabs.dispatch_event(mouse(
        MouseEventKind::Up(MouseButton::Left),
        first_width + 1,
    ));
    assert_eq!(tabs.apps[0].root, b);
    assert_eq!(tabs.active_app().root, a);
    assert!(tabs.drag_tab.is_none());
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn right_clicking_a_tab_opens_tab_context_menu() {
    let a = temp_dir("tab_menu_a");
    let b = temp_dir("tab_menu_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    tabs.strip_area = Rect::new(0, 0, 80, 1);
    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 2,
        row: 0,
        modifiers: KeyModifiers::empty(),
    }));
    assert_eq!(tabs.active, 0);
    assert!(
        matches!(tabs.active_app().context_menu.as_ref().map(|m| &m.target),
        Some(crate::app::ContextMenuTarget::Tab { root, index: 0 }) if root == &a)
    );
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn clicking_outside_tab_picker_closes_it() {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let root = temp_dir("picker_outside");
    let app = app_for(&root);
    let mut tabs = Tabs::new(vec![app], 0);
    tabs.tab_picker = Some(crate::search::TabPicker::new(&tabs.apps));
    tabs.tab_picker_area = ratatui::layout::Rect::new(10, 5, 40, 10);
    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 1,
        row: 1,
        modifiers: KeyModifiers::empty(),
    }));
    assert!(tabs.tab_picker.is_none());
    fs::remove_dir_all(root).ok();
}

#[test]
fn active_tab_stays_visible_while_switching_and_opening_tabs() {
    let dirs: Vec<_> = (0..10).map(|i| temp_dir(&format!("visible_{i}"))).collect();
    let apps: Vec<_> = dirs.iter().map(|dir| app_for(dir)).collect();
    let mut tabs = Tabs::new(apps, 0);
    tabs.strip_area = Rect::new(0, 0, 40, 1);

    for _ in 0..tabs.apps.len() * 2 {
        tabs.next_tab();
        assert!(crate::ui::tabstrip::is_tab_visible(
            &tabs,
            tabs.first_visible,
            tabs.active,
            tabs.strip_area.width
        ));
    }

    let new_dir = temp_dir("visible_new");
    tabs.open_tab(new_dir.clone()).unwrap();
    assert!(crate::ui::tabstrip::is_tab_visible(
        &tabs,
        tabs.first_visible,
        tabs.active,
        tabs.strip_area.width
    ));
    fs::remove_dir_all(&new_dir).ok();
    for dir in &dirs {
        fs::remove_dir_all(dir).ok();
    }
}

#[test]
fn tab_strip_clicks_and_wheel_scroll_horizontally() {
    let dirs: Vec<_> = (0..6).map(|i| temp_dir(&format!("scroll_{i}"))).collect();
    let apps: Vec<_> = dirs.iter().map(|dir| app_for(dir)).collect();
    let mut tabs = Tabs::new(apps, 2);
    tabs.strip_area = Rect::new(0, 0, 30, 1);
    tabs.first_visible = 2;

    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    }));
    assert_eq!(tabs.first_visible, 1);

    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 0,
        modifiers: KeyModifiers::empty(),
    }));
    assert_eq!(tabs.first_visible, 2);

    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 10,
        row: 0,
        modifiers: KeyModifiers::empty(),
    }));
    assert_eq!(tabs.first_visible, 1);
    let active = tabs.active;
    tabs.dispatch_event(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::empty(),
    }));
    assert_eq!(tabs.first_visible, 1);
    assert_eq!(tabs.active, active);
    assert!(tabs.active_app().context_menu.is_none());

    for dir in &dirs {
        fs::remove_dir_all(dir).ok();
    }
}

#[test]
fn close_tabs_to_right_keeps_the_target_tab_active() {
    let a = temp_dir("close_right_a");
    let b = temp_dir("close_right_b");
    let c = temp_dir("close_right_c");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b), app_for(&c)], 0);
    tabs.close_tabs_to_right(1);
    assert_eq!(tabs.apps.len(), 2);
    assert_eq!(tabs.active, 1);
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
    fs::remove_dir_all(&c).ok();
}

#[test]
fn close_other_tabs_preserves_only_the_target_tab() {
    let a = temp_dir("close_others_a");
    let b = temp_dir("close_others_b");
    let c = temp_dir("close_others_c");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b), app_for(&c)], 2);
    tabs.close_other_tabs(1);
    assert_eq!(tabs.apps.len(), 1);
    assert_eq!(tabs.active, 0);
    assert_eq!(tabs.active_app().root, b);
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
    fs::remove_dir_all(&c).ok();
}
