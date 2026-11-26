//! Key handling for the command palette and its routed sub-pickers.

use crossterm::event::KeyEvent;

use crate::config::static_keys;
use crate::list_picker::{handle_list_picker_key, OverlayKey};

use super::super::App;

impl App {
    /// Handles keyboard input while the command palette is open.
    ///
    /// Prefix routing: the first character of the query can be a prefix
    /// (`/` → files, `#` → content, `:` → go-to-line, `>` → commands)
    /// that delegates the remaining query to the corresponding sub-picker.
    /// Backspace on an empty routed query returns to commands mode.
    pub(super) fn handle_command_key(&mut self, key: KeyEvent) {
        // Detect Tab mode-toggle in the file/content routes (mirrors search overlay).
        if static_keys::is_toggle_modal(&key) {
            if let Some(ref mut p) = self.command_palette {
                if matches!(
                    p.route,
                    crate::command_palette::PaletteRoute::Files
                        | crate::command_palette::PaletteRoute::Content
                ) {
                    if let Some(ref mut s) = p.route_search {
                        s.toggle_mode();
                        p.route = match s.mode {
                            crate::search::SearchMode::Files => {
                                crate::command_palette::PaletteRoute::Files
                            }
                            crate::search::SearchMode::Content => {
                                crate::command_palette::PaletteRoute::Content
                            }
                        };
                    }
                }
            }
            return;
        }

        // Regex / case / whole-word toggles in the content route, the same
        // Ctrl+R / Ctrl+A / Ctrl+W the standalone search window had.
        if let Some(toggle) = static_keys::search_toggle(&key) {
            if let Some(search) = self
                .command_palette
                .as_mut()
                .filter(|p| p.route == crate::command_palette::PaletteRoute::Content)
                .and_then(|p| p.route_search.as_mut())
            {
                match toggle {
                    static_keys::SearchToggle::Regex => search.regex = !search.regex,
                    static_keys::SearchToggle::CaseSensitive => {
                        search.case_sensitive = !search.case_sensitive
                    }
                    static_keys::SearchToggle::WholeWord => search.whole_word = !search.whole_word,
                }
                search.refresh_now();
                return;
            }
        }

        // Capture route before key handling so we can detect a prefix transition.
        let route_before = self
            .command_palette
            .as_ref()
            .map(|p| p.route)
            .unwrap_or(crate::command_palette::PaletteRoute::Commands);

        // Use a block so the mutable borrow on command_palette is released
        // before we potentially call self methods in Handled/Activate arms.
        let action = self
            .command_palette
            .as_mut()
            .map(|p| handle_list_picker_key(p, &key));

        match action {
            Some(OverlayKey::Activate) => {
                let route = self
                    .command_palette
                    .as_ref()
                    .map(|p| p.route)
                    .unwrap_or(crate::command_palette::PaletteRoute::Commands);
                match route {
                    crate::command_palette::PaletteRoute::Commands => {
                        self.dispatch_command();
                    }
                    crate::command_palette::PaletteRoute::Files => {
                        self.dispatch_palette_file();
                    }
                    crate::command_palette::PaletteRoute::Content => {
                        self.dispatch_palette_content();
                    }
                    crate::command_palette::PaletteRoute::GotoLine => {
                        self.dispatch_palette_goto_line();
                    }
                    crate::command_palette::PaletteRoute::Symbols => {
                        self.dispatch_palette_symbol();
                    }
                    crate::command_palette::PaletteRoute::Diagnostics => {
                        self.dispatch_palette_diagnostic();
                    }
                }
            }
            Some(OverlayKey::Close) => {
                // Remember the search query for `search.keep_query`.
                if let Some(query) = self
                    .command_palette
                    .as_ref()
                    .and_then(|p| p.route_search.as_ref())
                    .map(|s| s.query.clone())
                {
                    self.last_search_query = query;
                }
                if let Some(ref mut p) = self.command_palette {
                    let in_routed_mode = p.route != crate::command_palette::PaletteRoute::Commands;
                    if in_routed_mode && p.opened_in_route {
                        // Opened by a shortcut straight into this mode: Esc
                        // closes the window, like every other popup.
                        self.command_palette = None;
                    } else if in_routed_mode {
                        p.route = crate::command_palette::PaletteRoute::Commands;
                        p.route_search = None;
                        p.route_goto_line = None;
                        p.route_symbols = None;
                        p.route_diagnostics = None;
                        p.selected = 0;
                        p.filtered = p.base_order.clone();
                        p.match_positions = vec![Vec::new(); p.filtered.len()];
                    } else {
                        self.command_palette = None;
                    }
                }
            }
            Some(OverlayKey::Handled) => {
                let current_route = self
                    .command_palette
                    .as_ref()
                    .map(|p| p.route)
                    .unwrap_or(crate::command_palette::PaletteRoute::Commands);
                self.maybe_create_palette_sub_picker(current_route, route_before);
            }
            _ => {}
        }
    }

    /// Creates the appropriate sub-picker when the palette route just changed
    /// (i.e. the user typed a prefix character). This is separated from
    /// `handle_command_key` to satisfy the borrow checker: the mutable borrow
    /// on `command_palette` must be released before calling `self` methods
    /// that read other `self` fields.
    fn maybe_create_palette_sub_picker(
        &mut self,
        current_route: crate::command_palette::PaletteRoute,
        previous_route: crate::command_palette::PaletteRoute,
    ) {
        if current_route == previous_route {
            return;
        }
        match current_route {
            crate::command_palette::PaletteRoute::Files => {
                let root = self.root.clone();
                let changed = self.git_changed_files_set();
                let s = crate::search::SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search.context_lines,
                    changed.as_ref(),
                );
                if let Some(ref mut p) = self.command_palette {
                    p.route_search = Some(s);
                }
            }
            crate::command_palette::PaletteRoute::Content => {
                let root = self.root.clone();
                let changed = self.git_changed_files_set();
                let mut s = crate::search::SearchState::new(
                    &root,
                    self.show_hidden,
                    self.ignore_gitignore,
                    self.config.search.context_lines,
                    changed.as_ref(),
                );
                s.toggle_mode();
                if let Some(ref mut p) = self.command_palette {
                    p.route_search = Some(s);
                }
            }
            crate::command_palette::PaletteRoute::GotoLine => {
                if let Some(ref mut p) = self.command_palette {
                    p.route_goto_line = Some(crate::search::GotoLineState::new());
                }
            }
            crate::command_palette::PaletteRoute::Symbols => {
                let symbols = self
                    .current_file
                    .as_deref()
                    .and_then(|path| self.symbols.get(path))
                    .cloned()
                    .unwrap_or_default();
                let picker = crate::search::SymbolPicker::new(
                    symbols,
                    Some(self.display_to_physical(self.active_line)),
                );
                if let Some(ref mut palette) = self.command_palette {
                    palette.route_symbols = Some(picker);
                }
            }
            crate::command_palette::PaletteRoute::Diagnostics => {
                let diagnostics = self
                    .current_file
                    .as_deref()
                    .and_then(|path| self.file_diagnostics.get(path))
                    .cloned()
                    .unwrap_or_default();
                let picker = crate::search::DiagnosticPicker::new(diagnostics);
                if let Some(ref mut palette) = self.command_palette {
                    palette.route_diagnostics = Some(picker);
                }
            }
            crate::command_palette::PaletteRoute::Commands => {}
        }
    }
}
