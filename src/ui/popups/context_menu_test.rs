use super::*;

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::{App, ContextActionId, ContextMenuEntry, ContextMenuState, ContextMenuTarget};
use crate::config::Config;

fn make_app(root: &std::path::Path) -> App {
    let cfg = Config {
        git: crate::config::GitConfig {
            status: false,
            ..Default::default()
        },
        ..Config::default()
    };
    App::new(root.to_path_buf(), cfg, None, None).unwrap()
}

fn menu(entries: Vec<ContextMenuEntry>) -> ContextMenuState {
    ContextMenuState {
        entries,
        selected: 0,
        anchor: (5, 5),
        target: ContextMenuTarget::Content,
    }
}

fn action(label: &str) -> ContextMenuEntry {
    ContextMenuEntry::Action {
        id: ContextActionId::CopyPath,
        label: label.to_string(),
    }
}

#[test]
fn menu_rect_sits_below_right_of_anchor() {
    let state = menu(vec![
        action("Open"),
        ContextMenuEntry::Separator,
        action("Copy absolute path"),
        action("Copy relative path"),
    ]);
    let area = Rect::new(0, 0, 80, 24);
    let r = menu_rect(&state, area);
    assert_eq!(r.x, 6, "menu must start one cell right of the anchor");
    assert_eq!(r.y, 6, "menu must start one cell below the anchor");
    assert_eq!(r.height, 4 + 2, "menu height is entries plus border");
    assert!(
        r.width >= 8 && r.x + r.width <= area.x + area.width,
        "menu must stay inside the screen"
    );
}

#[test]
fn menu_rect_clamps_to_area_left_and_top() {
    // Anchor at (0,0): x+1/y+1 still fits, so the menu starts there.
    let state = ContextMenuState {
        entries: vec![action("Copy")],
        selected: 0,
        anchor: (0, 0),
        target: ContextMenuTarget::Content,
    };
    let r = menu_rect(&state, Rect::new(0, 0, 80, 24));
    assert_eq!(r.x, 1);
    assert_eq!(r.y, 1);
}

#[test]
fn menu_rect_never_spills_past_bottom_right() {
    let state = menu(vec![action("Copy")]);
    let area = Rect::new(0, 0, 30, 10);
    let r = menu_rect(&state, area);
    assert!(
        r.x + r.width <= area.x + area.width,
        "menu right edge must stay on screen (x={}, w={}, area w={})",
        r.x,
        r.width,
        area.width
    );
    assert!(
        r.y + r.height <= area.y + area.height,
        "menu bottom edge must stay on screen"
    );
}

#[test]
fn menu_rect_sizes_width_from_longest_label() {
    let state = menu(vec![action("X"), action("Copy relative path")]);
    let area = Rect::new(0, 0, 80, 24);
    let r = menu_rect(&state, area);
    // Longest label "Copy relative path" (18 chars) + 2 padding + 2 borders.
    assert_eq!(r.width, 18 + 4);
}

#[test]
fn draw_context_menu_none_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.context_menu = None;
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| draw_context_menu(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();
}

#[test]
fn draw_context_menu_records_hit_area() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.context_menu = Some(menu(vec![
        action("Open"),
        ContextMenuEntry::Separator,
        action("Copy absolute path"),
    ]));
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| draw_context_menu(f, &mut app, Rect::new(0, 0, 80, 24)))
        .unwrap();

    let area = app.context_menu_area;
    assert_ne!(area.width, 0, "render must record the popup hit area");
    assert_eq!(area.height, 3 + 2, "hit area must match the popup height");
}
