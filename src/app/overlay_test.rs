use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::Overlay;
use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_overlay_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "x\n").unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

#[test]
fn a_fresh_app_has_no_overlay() {
    let root = temp_dir();
    let app = app_for(&root);
    assert!(app.open_overlays().is_empty());
    assert_eq!(app.top_overlay(), None);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_stack_lists_every_overlay_exactly_once() {
    let mut seen = std::collections::HashSet::new();
    for overlay in Overlay::STACK {
        assert!(
            seen.insert(format!("{overlay:?}")),
            "{overlay:?} listed twice"
        );
    }
    assert_eq!(seen.len(), 19);
}

#[test]
fn flag_overlays_are_reported_and_the_topmost_wins() {
    let root = temp_dir();
    let mut app = app_for(&root);

    app.show_help = true;
    assert_eq!(app.top_overlay(), Some(Overlay::Help));

    // Welcome is drawn above help, so it receives input first.
    app.show_welcome = true;
    assert_eq!(app.open_overlays(), vec![Overlay::Help, Overlay::Welcome]);
    assert_eq!(app.top_overlay(), Some(Overlay::Welcome));

    app.show_welcome = false;
    app.show_help = false;
    app.show_about = true;
    assert_eq!(app.top_overlay(), Some(Overlay::About));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_theme_picker_takes_input_over_a_stale_context_menu_and_help() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.theme_picker = Some(crate::search::ThemePicker::default());
    app.show_help = true;
    assert_eq!(app.top_overlay(), Some(Overlay::ThemePicker));
    app.theme_picker = None;
    assert_eq!(app.top_overlay(), Some(Overlay::Help));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn input_order_matches_draw_order_by_construction() {
    // `open_overlays` (draw order) and `top_overlay` (input) are both derived from
    // `Overlay::STACK`, so the topmost drawn overlay is always the one that
    // receives input.
    let root = temp_dir();
    let mut app = app_for(&root);
    app.theme_picker = Some(crate::search::ThemePicker::default());
    app.show_about = true;
    app.show_welcome = true;
    let open = app.open_overlays();
    assert_eq!(open.last().copied(), app.top_overlay());
    fs::remove_dir_all(&root).ok();
}
