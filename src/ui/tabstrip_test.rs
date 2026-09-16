use super::*;
use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::backend::TestBackend;

use crate::app::App;
use crate::config::Config;
use crate::theme::Theme;
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

fn render(tabs: &mut Tabs, area: Rect) -> Vec<String> {
    let backend = TestBackend::new(area.right(), area.bottom());
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let list = WorkspaceList::from_tabs(tabs);
    let theme = tabs.active_app().theme.clone();
    terminal
        .draw(|f| {
            draw_workspace_list(f, &theme, &list, area);
        })
        .unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        })
        .collect()
}

fn remove(dirs: &[std::path::PathBuf]) {
    for dir in dirs {
        fs::remove_dir_all(dir).ok();
    }
}

#[test]
fn draws_one_row_with_label_and_close_glyph_per_workspace() {
    let a = temp_dir("alpha");
    let b = temp_dir("beta");
    let mut tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let lines = render(&mut tabs, Rect::new(0, 0, 30, 4));
    assert!(lines[0].contains("Workspaces"));
    assert!(lines[1].contains("▸ alpha"), "active row: {:?}", lines[1]);
    assert!(lines[1].trim_end().ends_with("× │"));
    assert!(lines[2].contains("  beta"));
    assert!(lines[2].contains('×'));
    remove(&[a, b]);
}

#[test]
fn inactive_labels_and_close_glyphs_contrast_with_the_background_in_every_theme() {
    for (name, theme) in Theme::discover_all() {
        let background = Some(theme.background);
        assert_ne!(
            inactive_tab_style(&theme).fg,
            background,
            "inactive workspace labels are invisible in theme {name}"
        );
        assert_ne!(
            close_tab_style(&theme).fg,
            background,
            "workspace close glyphs are invisible in theme {name}"
        );
    }
}

#[test]
fn draws_the_new_tab_prompt_as_the_last_row() {
    let a = temp_dir("prompt");
    let mut tabs = Tabs::new(vec![app_for(&a)], 0);
    tabs.new_tab_prompt = Some("/some/path".to_string());
    let list = WorkspaceList::from_tabs(&tabs);
    let height = list.height_for(30);
    let lines = render(&mut tabs, Rect::new(0, 0, 40, height));
    assert_eq!(height, 4);
    assert!(lines[1].contains("prompt"));
    assert!(lines[2].contains("open: /some/path"));
    remove(&[a]);
}

#[test]
fn hit_test_maps_rows_to_switch_and_close() {
    let a = temp_dir("hit_a");
    let b = temp_dir("hit_b");
    let tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let area = Rect::new(0, 3, 30, 4);
    assert_eq!(hit_test(&tabs, area, 2, 4), Some(TabHit::Switch(0)));
    assert_eq!(hit_test(&tabs, area, 26, 4), Some(TabHit::Close(0)));
    assert_eq!(hit_test(&tabs, area, 5, 5), Some(TabHit::Switch(1)));
    assert_eq!(hit_test(&tabs, area, 27, 5), Some(TabHit::Close(1)));
    remove(&[a, b]);
}

#[test]
fn hit_test_ignores_borders_empty_rows_and_outside_clicks() {
    let a = temp_dir("outside_a");
    let b = temp_dir("outside_b");
    let tabs = Tabs::new(vec![app_for(&a), app_for(&b)], 0);
    let area = Rect::new(0, 0, 30, 6);
    assert_eq!(hit_test(&tabs, area, 5, 0), None);
    assert_eq!(hit_test(&tabs, area, 0, 1), None);
    assert_eq!(hit_test(&tabs, area, 5, 3), None);
    assert_eq!(hit_test(&tabs, area, 5, 5), None);
    assert_eq!(hit_test(&tabs, area, 5, 9), None);
    assert_eq!(hit_test(&tabs, area, 40, 1), None);
    remove(&[a, b]);
}

#[test]
fn long_labels_are_truncated_to_the_row_width() {
    let long_name = "a".repeat(40);
    let base = temp_dir("base");
    let dir = base.join(&long_name);
    fs::create_dir_all(&dir).unwrap();
    let other = temp_dir("other");
    assert!(tab_label_with_max(&app_for(&dir), 10).ends_with('…'));
    let mut tabs = Tabs::new(vec![app_for(&dir), app_for(&other)], 0);
    let lines = render(&mut tabs, Rect::new(0, 0, 20, 4));
    assert!(lines[1].contains('…'), "{:?}", lines[1]);
    assert!(lines[1].contains('×'));
    assert_eq!(lines[1].chars().count(), 20);
    remove(&[base, other]);
}

#[test]
fn duplicate_root_names_use_the_shortest_unique_suffix() {
    let parent =
        std::env::temp_dir().join(format!("mantis_tabstrip_collision_{}", std::process::id()));
    let api = parent.join("api").join("repo");
    let web = parent.join("web").join("repo");
    fs::create_dir_all(&api).unwrap();
    fs::create_dir_all(&web).unwrap();
    let tabs = Tabs::new(vec![app_for(&api), app_for(&web)], 0);
    assert_eq!(unique_tab_label(&tabs, 0), "api/repo");
    assert_eq!(unique_tab_label(&tabs, 1), "web/repo");
    fs::remove_dir_all(&parent).ok();
}

#[test]
fn git_branch_and_changed_count_are_shown_when_the_row_has_room() {
    let root = temp_dir("git_badge");
    let second_root = temp_dir("git_badge_second");
    let mut app = app_for(&root);
    app.git_info = Some(crate::git::GitRepoInfo {
        head: crate::git::GitHead::Branch("main".into()),
        ahead: 0,
        behind: 0,
        total_changed: 3,
        staged: 0,
        untracked: 0,
    });
    let mut tabs = Tabs::new(vec![app, app_for(&second_root)], 0);
    let wide = render(&mut tabs, Rect::new(0, 0, 40, 4));
    assert!(wide[1].contains("git_badge ·main ●3"), "{:?}", wide[1]);
    let narrow = render(&mut tabs, Rect::new(0, 0, 18, 4));
    assert!(!narrow[1].contains("main"), "{:?}", narrow[1]);
    remove(&[root, second_root]);
}

#[test]
fn overflowing_lists_show_scroll_affordances_on_the_borders() {
    let dirs: Vec<_> = (0..6).map(|i| temp_dir(&format!("scroll_{i}"))).collect();
    let apps: Vec<_> = dirs.iter().map(|dir| app_for(dir)).collect();
    let mut tabs = Tabs::new(apps, 2);
    let area = Rect::new(0, 0, 30, 5);

    tabs.first_visible = 0;
    let top = render(&mut tabs, area);
    assert!(!top[0].contains('▴'));
    assert!(top[4].contains('▾'));
    assert_eq!(hit_test(&tabs, area, 5, 0), None);
    assert_eq!(hit_test(&tabs, area, 5, 4), Some(TabHit::ScrollDown));

    tabs.first_visible = 2;
    let middle = render(&mut tabs, area);
    assert!(middle[0].contains('▴'));
    assert!(middle[1].contains("scroll_2"));
    assert!(middle[4].contains('▾'));
    assert_eq!(hit_test(&tabs, area, 5, 0), Some(TabHit::ScrollUp));
    assert_eq!(hit_test(&tabs, area, 5, 1), Some(TabHit::Switch(2)));

    remove(&dirs);
}

#[test]
fn list_height_is_capped_to_a_third_of_the_column() {
    let dirs: Vec<_> = (0..12).map(|i| temp_dir(&format!("cap_{i}"))).collect();
    let apps: Vec<_> = dirs.iter().map(|dir| app_for(dir)).collect();
    let tabs = Tabs::new(apps, 0);
    let list = WorkspaceList::from_tabs(&tabs);
    assert_eq!(list.height_for(60), 14);
    assert_eq!(list.height_for(30), 10);
    assert_eq!(list.height_for(6), 3);
    remove(&dirs);
}

#[test]
fn scroll_offset_keeps_the_active_row_in_view_without_blank_rows() {
    assert_eq!(scroll_offset(0, 7, 10, 3), 5);
    assert_eq!(scroll_offset(5, 1, 10, 3), 1);
    assert_eq!(scroll_offset(9, 9, 10, 3), 7);
    assert_eq!(scroll_offset(4, 0, 2, 5), 0);
    for active in 0..10 {
        let first = scroll_offset(0, active, 10, 4);
        assert!(is_tab_visible(first, active, 4));
    }
}
