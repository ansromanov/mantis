use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::App;
use crate::command_palette::CommandPalette;
use crate::config::Config;
use crate::ui::popups::draw_command_palette;

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

#[test]
fn pinned_entries_show_star_prefix_when_query_empty() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    // Build a palette with the help action pinned.
    let help_idx = crate::command_palette::COMMANDS
        .iter()
        .position(|c| c.action_id == "help")
        .expect("help must be in COMMANDS");
    let mut order: Vec<usize> = vec![help_idx];
    for i in 0..crate::command_palette::COMMANDS.len() {
        if i != help_idx {
            order.push(i);
        }
    }
    let keymap = app.keys().clone();
    app.command_palette = Some(CommandPalette::new(&keymap, order, 1));

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_command_palette(f, &mut app, f.area()))
        .unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        joined.contains('★'),
        "pinned entry must render with ★ prefix when query is empty"
    );
}

#[test]
fn star_prefix_hidden_when_query_is_typed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = make_app(dir.path());
    let help_idx = crate::command_palette::COMMANDS
        .iter()
        .position(|c| c.action_id == "help")
        .expect("help must be in COMMANDS");
    let mut order: Vec<usize> = vec![help_idx];
    for i in 0..crate::command_palette::COMMANDS.len() {
        if i != help_idx {
            order.push(i);
        }
    }
    let keymap = app.keys().clone();
    let mut palette = CommandPalette::new(&keymap, order, 1);
    // Typing a query disables the pin prefix.
    palette.push('h');
    app.command_palette = Some(palette);

    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| draw_command_palette(f, &mut app, f.area()))
        .unwrap();
    let joined = buffer_rows(&terminal).join("\n");
    assert!(
        !joined.contains('★'),
        "star prefix must not appear when a query is active"
    );
}
