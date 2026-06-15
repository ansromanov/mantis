use super::*;

#[test]
fn command_palette_starts_with_all_commands() {
    let p = CommandPalette::default();
    assert_eq!(p.results_len(), COMMANDS.len());
    assert_eq!(p.selected, 0);
}

#[test]
fn command_palette_push_filters() {
    let mut p = CommandPalette::default();
    p.push('w');
    assert!(p.results_len() < COMMANDS.len());
    let cmd = p.selected_command().unwrap();
    assert!(cmd.name.to_lowercase().contains('w'));
}

#[test]
fn command_palette_pop_restores() {
    let mut p = CommandPalette::default();
    p.push('z');
    let after_push = p.results_len();
    p.pop();
    assert_eq!(p.results_len(), COMMANDS.len());
    assert!(after_push < COMMANDS.len());
}

#[test]
fn command_palette_selected_command() {
    let p = CommandPalette::default();
    let first = p.selected_command();
    assert!(first.is_some());
    assert_eq!(first.unwrap().name, COMMANDS[0].name);
}

#[test]
fn command_palette_selected_index_returns_none_when_empty() {
    let mut p = CommandPalette::default();
    for c in "zzzzzzzzzzzzz".chars() {
        p.push(c);
    }
    assert_eq!(p.results_len(), 0);
    assert!(p.selected_index().is_none());
    assert!(p.selected_command().is_none());
}

#[test]
fn command_palette_results_len() {
    let p = CommandPalette::default();
    assert_eq!(p.results_len(), COMMANDS.len());
}

#[test]
fn command_palette_filters_by_name() {
    let mut p = CommandPalette::default();
    for c in "markdown".chars() {
        p.push(c);
    }
    assert!(p.selected_command().unwrap().name.contains("markdown"));
}

#[test]
fn command_palette_filters_by_keybinding() {
    let mut p = CommandPalette::default();
    for c in "ctrl+g".chars() {
        p.push(c);
    }
    let cmd = p.selected_command().unwrap();
    assert_eq!(cmd.action_id, "toggle_git_mode");
}
