use super::*;

use ratatui::{backend::TestBackend, Terminal};

#[test]
fn draw_worktree_picker_records_list_area() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = crate::app::App::new(
        dir.path().to_path_buf(),
        crate::config::Config::default(),
        None,
        None,
    )
    .unwrap();
    app.worktree_picker = Some(crate::search::WorktreePicker::for_test(Vec::new()));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| draw_worktree_picker(f, &mut app, f.area()))
        .unwrap();
    assert!(app.worktree_picker_area.height > 0);
}
