use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::backend::TestBackend;

use crate::app::App;
use crate::config::Config;
use crate::workspace::Tabs;

/// Creates a temp directory whose *basename* is short (`name`, deduped with a
/// counter) so the tab label isn't truncated — the tests assert on the label
/// text and its column width. Isolation comes from a unique parent dir.
fn temp_dir(name: &str) -> std::path::PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let parent = std::env::temp_dir().join(format!("mantis_tabstrip_{}_{n}", std::process::id()));
    let dir = parent.join(name);
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn render(tabs: &mut Tabs, area: Rect) -> String {
    let backend = TestBackend::new(area.width, area.height.max(1));
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    terminal.draw(|f| draw_tabstrip(f, tabs, area)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol())
        .collect()
}

#[test]
fn draws_a_label_and_close_glyph_for_each_tab() {
    let a = temp_dir("alpha");
    let b = temp_dir("beta");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let area = Rect::new(0, 0, 60, 1);
    let text = render(&mut tabs, area);
    assert!(text.contains(a.file_name().unwrap().to_str().unwrap()));
    assert!(text.contains(b.file_name().unwrap().to_str().unwrap()));
    assert!(text.contains('×'));
    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn draws_the_new_tab_prompt_instead_of_labels_when_open() {
    let a = temp_dir("prompt");
    let mut tabs = Tabs::new(vec![app_for(&a)], 0);
    tabs.new_tab_prompt = Some("/some/path".to_string());
    let area = Rect::new(0, 0, 60, 1);
    let text = render(&mut tabs, area);
    assert!(text.contains("open project"));
    assert!(text.contains("/some/path"));
    fs::remove_dir_all(&a).ok();
}

#[test]
fn hit_test_maps_clicks_to_switch_and_close() {
    let a = temp_dir("hit_a");
    let b = temp_dir("hit_b");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let area = Rect::new(0, 0, 60, 1);
    // Force a render so build_segments' width math matches what's on screen
    // (labels are directory-name-derived, deterministic either way).
    render(&mut tabs, area);

    // Layout per tab is " {label} × " → total width = label chars + 4, with
    // the close glyph occupying the last two columns of that span.
    let label_len = tab_label(tabs.active_app()).chars().count() as u16;
    let width = label_len + 4;
    // A column inside the label region resolves to Switch for that tab.
    assert_eq!(hit_test(&tabs, area, 1, 0), Some(TabHit::Switch(0)));
    // A column in the last two of the span resolves to Close.
    assert_eq!(hit_test(&tabs, area, width - 1, 0), Some(TabHit::Close(0)));
    // The second tab's label region resolves to Switch(1).
    assert_eq!(hit_test(&tabs, area, width + 1, 0), Some(TabHit::Switch(1)));

    fs::remove_dir_all(&a).ok();
    fs::remove_dir_all(&b).ok();
}

#[test]
fn hit_test_returns_none_outside_the_strip_row() {
    let a = temp_dir("outside");
    let tabs = Tabs::new(vec![app_for(&a)], 0);
    let area = Rect::new(0, 0, 60, 1);
    assert_eq!(hit_test(&tabs, area, 1, 5), None);
    fs::remove_dir_all(&a).ok();
}

#[test]
fn tab_label_truncates_long_directory_names() {
    let long_name = "a".repeat(40);
    let dir = temp_dir("base").join(&long_name);
    fs::create_dir_all(&dir).unwrap();
    let app = app_for(&dir);
    let label = tab_label(&app);
    assert!(label.chars().count() <= MAX_LABEL_LEN);
    assert!(label.ends_with('…'));
}
