use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use crossterm::event::KeyModifiers;

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
