use super::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A test double that records all calls and provides basic list-picker
/// behaviour for testing the shared dispatcher.
struct TestPicker {
    query: String,
    items: Vec<String>,
    selected: usize,
    push_count: usize,
    pop_count: usize,
}

impl TestPicker {
    fn new(items: Vec<String>) -> Self {
        TestPicker {
            query: String::new(),
            items,
            selected: 0,
            push_count: 0,
            pop_count: 0,
        }
    }
}

impl ListPicker for TestPicker {
    fn query_push(&mut self, c: char) {
        self.query.push(c);
        self.push_count += 1;
    }
    fn query_pop(&mut self) {
        self.query.pop();
        self.pop_count += 1;
    }
    fn query_is_empty(&self) -> bool {
        self.query.is_empty()
    }
    fn results_len(&self) -> usize {
        self.items.len()
    }
    fn selected(&self) -> usize {
        self.selected
    }
    fn set_selected(&mut self, i: usize) {
        self.selected = i;
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

#[test]
fn esc_returns_close() {
    let mut p = TestPicker::new(vec!["a".into()]);
    assert_eq!(
        handle_list_picker_key(&mut p, &key(KeyCode::Esc)),
        OverlayKey::Close
    );
}

#[test]
fn enter_returns_activate() {
    let mut p = TestPicker::new(vec!["a".into()]);
    assert_eq!(
        handle_list_picker_key(&mut p, &key(KeyCode::Enter)),
        OverlayKey::Activate
    );
}

#[test]
fn backspace_non_empty_pops() {
    let mut p = TestPicker::new(vec!["a".into()]);
    p.query_push('x');
    assert!(!p.query_is_empty());
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Backspace));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.pop_count, 1);
    assert!(p.query_is_empty());
}

#[test]
fn backspace_empty_returns_close() {
    let mut p = TestPicker::new(vec!["a".into()]);
    assert!(p.query_is_empty());
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Backspace));
    assert_eq!(result, OverlayKey::Close);
    assert_eq!(p.pop_count, 0);
    assert!(p.query_is_empty());
}

#[test]
fn char_pushes_to_query() {
    let mut p = TestPicker::new(vec!["a".into()]);
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Char('x')));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.push_count, 1);
    assert_eq!(p.query, "x");
}

#[test]
fn up_navigates_with_clamping() {
    let mut p = TestPicker::new(vec!["a".into(), "b".into(), "c".into()]);
    p.selected = 1;
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Up));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.selected, 0);
    // At top: should stay at 0
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Up));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.selected, 0);
}

#[test]
fn down_navigates_with_clamping() {
    let mut p = TestPicker::new(vec!["a".into(), "b".into(), "c".into()]);
    p.selected = 1;
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Down));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.selected, 2);
    // At bottom: should stay at 2
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Down));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.selected, 2);
}

#[test]
fn down_on_empty_list_stays_put() {
    let mut p = TestPicker::new(vec![]);
    // results_len is 0, Down check `0 + 1 < 0` fails
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Down));
    assert_eq!(result, OverlayKey::Handled);
    assert_eq!(p.selected, 0);
}

#[test]
fn unknown_key_returns_pass() {
    let mut p = TestPicker::new(vec!["a".into()]);
    let result = handle_list_picker_key(&mut p, &key(KeyCode::F(1)));
    assert_eq!(result, OverlayKey::Pass);
    assert_eq!(p.push_count, 0);
    assert_eq!(p.pop_count, 0);
}

#[test]
fn plugin_picker_impl_backspace_closes() {
    // PluginPicker has no query, so query_is_empty always true.
    // Backspace should therefore return Close.
    let mut p = crate::search::PluginPicker::new(vec![]);
    assert!(p.query_is_empty());
    let result = handle_list_picker_key(&mut p, &key(KeyCode::Backspace));
    assert_eq!(result, OverlayKey::Close);
}

#[test]
fn plugin_picker_impl_push_pop_noop() {
    let mut p = crate::search::PluginPicker::new(vec![(
        "a".into(),
        true,
        crate::plugin::PluginKind::Process,
    )]);
    p.query_push('x');
    assert!(p.query_is_empty());
    p.query_pop();
    assert!(p.query_is_empty());
}

#[test]
fn tree_filter_impl_up_down_noop() {
    let mut f = crate::search::TreeFilter::new();
    f.query_push('a');
    assert_eq!(f.results_len(), 0);
    f.set_selected(42);
    assert_eq!(f.selected(), 0);
}

#[test]
fn goto_line_impl_up_down_noop() {
    let mut g = crate::search::GotoLineState::new();
    g.query_push('5');
    assert_eq!(g.results_len(), 0);
    g.set_selected(42);
    assert_eq!(g.selected(), 0);
}
