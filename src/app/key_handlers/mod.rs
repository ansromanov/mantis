mod editor;
mod normal;
mod overlay;
mod visual;

use super::App;

impl App {
    /// Dispatches a key event. Overlays (help, theme, history, search) are
    /// checked first; otherwise normal tree/content key handling applies.
    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) {
        use crossterm::event::KeyEventKind;

        // The Windows console backend reports both a Press and a Release event
        // for a single physical key press (Unix only reports Press unless the
        // kitty protocol is enabled). Ignore Release so every action runs once
        // rather than twice. `Repeat` is kept so held keys still navigate.
        if key.kind == KeyEventKind::Release {
            return;
        }
        if self.show_about {
            match key.code {
                crossterm::event::KeyCode::Char('?')
                | crossterm::event::KeyCode::Esc
                | crossterm::event::KeyCode::Char('q') => {
                    self.show_about = false;
                }
                crossterm::event::KeyCode::Enter => {
                    self.open_release_url();
                }
                _ => {}
            }
            return;
        }
        if self.show_help {
            if matches!(
                key.code,
                crossterm::event::KeyCode::Char('?')
                    | crossterm::event::KeyCode::Esc
                    | crossterm::event::KeyCode::Char('q')
            ) {
                self.show_help = false;
            }
            return;
        }
        if self.theme_picker.is_some() {
            self.handle_theme_key(key);
        } else if self.command_palette.is_some() {
            self.handle_command_key(key);
        } else if self.history.is_some() {
            self.handle_history_key(key);
        } else if self.search.is_some() {
            self.handle_search_key(key);
        } else if self.in_file_search.is_some() {
            self.handle_in_file_search_key(key);
        } else {
            self.handle_normal_key(key);
        }
    }
}
