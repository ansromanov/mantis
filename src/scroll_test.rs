use super::*;

#[test]
fn test_scroll_state_new() {
    let state = ScrollState::new();
    assert_eq!(state.scroll, 0);
}

#[test]
fn test_scroll_up() {
    let mut state = ScrollState::new();
    state.scroll = 10;
    state.scroll_up(3);
    assert_eq!(state.scroll, 7);
    state.scroll_up(10);
    assert_eq!(state.scroll, 0);
}

#[test]
fn test_scroll_down() {
    let mut state = ScrollState::new();
    state.scroll = 5;
    state.scroll_down(3, 10);
    assert_eq!(state.scroll, 8);
    state.scroll_down(5, 10);
    assert_eq!(state.scroll, 10);
}

#[test]
fn test_scroll_clamp() {
    let mut state = ScrollState::new();
    state.scroll = 15;
    state.clamp(10);
    assert_eq!(state.scroll, 10);
    state.clamp(20);
    assert_eq!(state.scroll, 10);
}
