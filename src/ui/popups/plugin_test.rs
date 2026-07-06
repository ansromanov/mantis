use crate::app::App;
use crate::config::Config;
use crate::ui::popups::draw_plugin_picker;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

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

fn buffer_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buf = terminal.backend().buffer();
    let area = buf.area;
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect()
        })
        .collect()
}

// ── draw_plugin_picker ──────────────────────────────────────────────────

#[test]
fn draw_plugin_picker_empty_list() {
    use crate::search::PluginPicker;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.plugin_picker = Some(PluginPicker::new(vec![]));
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_plugin_picker(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Plugins"));
    assert!(joined.contains("Space/Enter"));
}

#[test]
fn draw_plugin_picker_with_entries_shows_names_and_state() {
    use crate::search::PluginPicker;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.plugin_picker = Some(PluginPicker::new(vec![
        (
            "alpha".to_string(),
            true,
            crate::plugin::PluginKind::Process,
            None,
        ),
        (
            "beta".to_string(),
            false,
            crate::plugin::PluginKind::Process,
            None,
        ),
    ]));
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_plugin_picker(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("Plugins"));
    assert!(joined.contains("alpha"));
    assert!(joined.contains("beta"));
    // geometry must be recorded for mouse hit-testing
    assert!(app.plugin_picker_area.width > 0);
}

#[test]
fn draw_plugin_picker_shows_crash_badge_for_dead_plugin() {
    use crate::search::PluginPicker;
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.plugin_picker = Some(PluginPicker::new(vec![(
        "crashy".to_string(),
        false,
        crate::plugin::PluginKind::Process,
        Some("panic: oh no (log: /tmp/crashy.log)".to_string()),
    )]));
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_plugin_picker(f, &mut app, f.area()))
        .unwrap();
    let rows = buffer_rows(&terminal);
    let joined = rows.join("\n");
    assert!(joined.contains("crashy"));
    assert!(joined.contains("panic: oh no"));
}

#[test]
fn draw_plugin_picker_none_returns_early() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    app.plugin_picker = None;
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_plugin_picker(f, &mut app, f.area()))
        .unwrap();
}

// Modified for test requirements
