use super::*;
use crate::app::App;
use crate::config::Config;
use std::path::PathBuf;

#[test]
fn draw_revision_picker_renders_with_items() {
    let mut app = App::new(PathBuf::from("/nonexistent"), Config::default(), None, None).unwrap();
    app.revision_picker = Some(crate::search::RevisionPicker::for_test(vec![
        crate::search::RevisionItem {
            rev: "HEAD".into(),
            display: "HEAD (current)".into(),
        },
        crate::search::RevisionItem {
            rev: "abc1234".into(),
            display: "abc1234 Fix the thing".into(),
        },
    ]));

    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| draw_revision_picker(f, &mut app, f.area()))
        .unwrap();
}
