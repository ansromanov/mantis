use super::*;

use std::fs;

use ratatui::{backend::TestBackend, Terminal};

use crate::{config::Config, search::TabPicker};

#[test]
fn renders_tab_picker_root_rows() {
    let root = std::env::temp_dir().join(format!("mantis_tab_popup_{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let app = crate::app::App::new(root.clone(), Config::default(), None, None).unwrap();
    let mut tabs = Tabs::new(vec![app], 0);
    tabs.tab_picker = Some(TabPicker::new(&tabs.apps));
    let backend = TestBackend::new(80, 20);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| draw_tab_picker(frame, &mut tabs))
        .unwrap();
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("Open tabs"));
    assert!(text.contains("1/1"));
    fs::remove_dir_all(&root).ok();
}
