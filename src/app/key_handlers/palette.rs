//! Command-palette routes (file, content, go-to-line, symbol, diagnostic) and theme application.

use crate::highlight::Highlighter;
use crate::theme::{Theme, ThemeConfig};

use super::super::{diff_line_style, App};

impl App {
    /// A fresh command palette with the usage-ranked command order and the
    /// current applicability reasons. Every way of opening the palette goes
    /// through here so the command list looks the same however it was reached.
    pub(crate) fn new_palette(&self) -> crate::command_palette::CommandPalette {
        let (base_order, base_pinned) = crate::command_palette::ranked_base_order(
            &self.command_usage,
            self.config.palette_pin_recent,
            self.config.palette_frequent_count,
        );
        let reasons = crate::command_palette::COMMANDS
            .iter()
            .map(|command| self.check_applicability(&command.action_id).err())
            .collect();
        crate::command_palette::CommandPalette::new(&self.keys, base_order, base_pinned, reasons)
    }

    /// A project search state for the palette's file (`content == false`) or
    /// content route, restoring the last query when `search.keep_query` is on.
    pub(crate) fn new_search_state(&self, content: bool) -> crate::search::SearchState {
        let changed = self.git_changed_files_set();
        let mut search = crate::search::SearchState::new(
            &self.root,
            self.show_hidden,
            self.ignore_gitignore,
            self.config.search.context_lines,
            changed.as_ref(),
        );
        if content {
            search.toggle_mode();
        }
        if self.config.search.keep_query && !self.last_search_query.is_empty() {
            search.query = self.last_search_query.clone();
            search.refresh_now();
        }
        search
    }

    /// Opens the palette directly in the file or content route (picked from
    /// `search.mode`). This is the single search window: `Ctrl+T`, `Ctrl+F`,
    /// the menu, the context menu and `/` in the palette all end up here.
    pub(crate) fn open_palette_search(&mut self, search: crate::search::SearchState) {
        let mut palette = self.new_palette();
        palette.route = match search.mode {
            crate::search::SearchMode::Files => crate::command_palette::PaletteRoute::Files,
            crate::search::SearchMode::Content => crate::command_palette::PaletteRoute::Content,
        };
        palette.route_search = Some(search);
        palette.opened_in_route = true;
        self.command_palette = Some(palette);
    }

    /// Opens project-wide content search in the palette.
    pub(crate) fn open_content_search(&mut self) {
        let search = self.new_search_state(true);
        self.open_palette_search(search);
    }

    /// Opens go-to-line in the palette. Works from either pane: the line is
    /// resolved against the open file, so there is no need to focus it first.
    pub(crate) fn open_palette_goto_line(&mut self) {
        if self.current_file.is_none() && self.content.is_empty() {
            self.set_status("go to line: no file is open");
            return;
        }
        let mut palette = self.new_palette();
        palette.route = crate::command_palette::PaletteRoute::GotoLine;
        palette.route_goto_line = Some(crate::search::GotoLineState::new());
        palette.opened_in_route = true;
        self.command_palette = Some(palette);
    }

    /// Opens the file selected in the palette's file-search route and closes
    /// the palette. Mirrors `activate_search_selection` but reads from the
    /// palette's sub-picker rather than `self.search`.
    pub(crate) fn dispatch_palette_file(&mut self) {
        let action = self.command_palette.as_ref().and_then(|p| {
            p.route_search
                .as_ref()
                .and_then(|s| s.file_results.get(s.selected).cloned())
        });
        self.command_palette = None;
        if let Some(path) = action {
            self.open_file(&path);
            self.reveal_in_tree(&path);
        }
    }

    /// Opens the file at the content-match line selected in the palette's
    /// content-search route and closes the palette.
    pub(crate) fn dispatch_palette_content(&mut self) {
        let action = self.command_palette.as_ref().and_then(|p| {
            p.route_search.as_ref().and_then(|s| {
                s.content_results
                    .get(s.selected)
                    .map(|m| (m.path.clone(), m.line_num))
            })
        });
        self.command_palette = None;
        if let Some((path, line)) = action {
            self.open_file(&path);
            self.set_content_scroll(line.saturating_sub(1));
            self.reveal_in_tree(&path);
        }
    }

    /// Jumps to the line number typed in the palette's go-to-line route and
    /// closes the palette.
    pub(crate) fn dispatch_palette_goto_line(&mut self) {
        let query = self
            .command_palette
            .as_ref()
            .and_then(|p| p.route_goto_line.as_ref().map(|g| g.query.clone()));
        self.command_palette = None;
        if let Some(q) = query {
            self.goto_line_from_query(&q);
        }
    }

    /// Opens the symbol-outline picker, seeded at the symbol under the cursor.
    pub(crate) fn open_symbol_outline(&mut self) {
        let Some(path) = self.current_file.as_ref() else {
            self.set_status("symbol outline: no file is open");
            return;
        };
        let Some(symbols) = self.symbols.get(path).cloned().filter(|s| !s.is_empty()) else {
            self.set_status("symbol outline: no symbols in file");
            return;
        };
        let mut palette = self.new_palette();
        palette.opened_in_route = true;
        palette.route = crate::command_palette::PaletteRoute::Symbols;
        palette.route_symbols = Some(crate::search::SymbolPicker::new(
            symbols,
            Some(self.display_to_physical(self.active_line)),
        ));
        self.command_palette = Some(palette);
    }

    /// Jumps to the selected symbol's source line and closes the symbol picker.
    pub(crate) fn dispatch_palette_symbol(&mut self) {
        let line = self
            .command_palette
            .as_ref()
            .and_then(|palette| palette.route_symbols.as_ref())
            .and_then(crate::search::SymbolPicker::selected_symbol)
            .map(|symbol| symbol.line);
        self.command_palette = None;
        if let Some(line) = line {
            let display_line = self.physical_to_display(line);
            let max = self.display_line_count().saturating_sub(1);
            self.active_line = display_line.min(max);
            self.scroll_active_line_into_view();
            self.mark_content_scrolled();
            self.mark_session_dirty();
        }
    }

    /// Opens the diagnostic picker for the current file.
    pub(crate) fn open_diagnostics_picker(&mut self) {
        let Some(path) = self.current_file.as_ref() else {
            self.set_status("diagnostics: no file is open");
            return;
        };
        let Some(diagnostics) = self
            .file_diagnostics
            .get(path)
            .cloned()
            .filter(|diagnostics| !diagnostics.is_empty())
        else {
            self.set_status("diagnostics: no diagnostics in file");
            return;
        };
        let mut palette = self.new_palette();
        palette.opened_in_route = true;
        palette.route = crate::command_palette::PaletteRoute::Diagnostics;
        palette.route_diagnostics = Some(crate::search::DiagnosticPicker::new(diagnostics));
        self.command_palette = Some(palette);
    }

    /// Jumps to the selected diagnostic's source line and closes the picker.
    pub(crate) fn dispatch_palette_diagnostic(&mut self) {
        let line = self
            .command_palette
            .as_ref()
            .and_then(|palette| palette.route_diagnostics.as_ref())
            .and_then(crate::search::DiagnosticPicker::selected_diagnostic)
            .map(|diagnostic| diagnostic.line);
        self.command_palette = None;
        if let Some(line) = line {
            let display_line = self.physical_to_display(line);
            let max = self.display_line_count().saturating_sub(1);
            self.active_line = display_line.min(max);
            self.scroll_active_line_into_view();
            self.mark_content_scrolled();
            self.mark_session_dirty();
        }
    }

    /// Parses a goto-line query and jumps to the resulting line. Supports
    /// absolute (`42`, 1-indexed, clamped to the file), relative forward
    /// (`+5`), and relative backward (`-3`) jumps. Diff and rendered
    /// views have no active-line cursor, so relative jumps are based on
    /// `content_scroll` there instead. Shared by the standalone goto-line
    /// dialog and the palette's `:` route.
    pub(crate) fn goto_line_from_query(&mut self, q: &str) {
        let has_cursor = self.has_text_cursor();
        let base = if has_cursor {
            self.active_line
        } else {
            self.content_scroll
        };
        let target = if q.is_empty() {
            None
        } else if let Some(offset) = q.strip_prefix('+') {
            offset.parse::<usize>().ok().map(|n| base.saturating_add(n))
        } else if let Some(offset) = q.strip_prefix('-') {
            offset.parse::<usize>().ok().map(|n| base.saturating_sub(n))
        } else {
            // 1-indexed → 0-indexed
            q.parse::<usize>().ok().map(|n| n.saturating_sub(1))
        };
        if let Some(line) = target {
            let max = self.display_line_count().saturating_sub(1);
            let line = line.min(max);
            if has_cursor {
                self.active_line = line;
                self.scroll_active_line_into_view();
            } else {
                self.set_content_scroll(line);
            }
            self.mark_content_scrolled();
            self.mark_session_dirty();
        }
    }

    /// Applies the theme selected in the picker, saves it to config, and
    /// closes the overlay.
    pub(crate) fn apply_selected_theme(&mut self) {
        let name = self
            .theme_picker
            .as_ref()
            .and_then(|p| p.selected_name())
            .map(String::from);
        self.theme_picker = None;
        if let Some(ref name) = name {
            if let Some(theme) = Theme::load(name) {
                self.apply_theme(name, theme);
                self.config.theme = ThemeConfig::from_preset(name);
                self.save_config();
            }
        }
    }

    /// Switches the active theme and re-renders the current view with it,
    /// preserving scroll position.
    pub(crate) fn apply_theme(&mut self, _theme_name: &str, theme: Theme) {
        self.rendered_content.clear();
        self.rendered_text.clear();
        self.theme = theme;
        self.highlighter = Highlighter::new(&self.theme.syntax);
        self.loader_set_theme();
        if self.is_diff {
            self.highlighted = self
                .content
                .iter()
                .map(|l| vec![(diff_line_style(l, &self.theme), l.clone())])
                .collect();
        } else if let Some(path) = self.current_file.clone() {
            self.reopen_file(&path);
        }
    }
}

#[cfg(test)]
impl App {
    /// The palette's file/content search, when the palette is open in a search route.
    pub(crate) fn palette_search(&self) -> Option<&crate::search::SearchState> {
        self.command_palette
            .as_ref()
            .and_then(|p| p.route_search.as_ref())
    }

    /// Mutable access to the palette's file/content search.
    pub(crate) fn palette_search_mut(&mut self) -> Option<&mut crate::search::SearchState> {
        self.command_palette
            .as_mut()
            .and_then(|p| p.route_search.as_mut())
    }

    /// The palette's go-to-line input, when the palette is open in that route.
    pub(crate) fn palette_goto_line(&self) -> Option<&crate::search::GotoLineState> {
        self.command_palette
            .as_ref()
            .and_then(|p| p.route_goto_line.as_ref())
    }
}

#[cfg(test)]
impl App {
    /// Mutable access to the palette's go-to-line input.
    pub(crate) fn palette_goto_line_mut(&mut self) -> Option<&mut crate::search::GotoLineState> {
        self.command_palette
            .as_mut()
            .and_then(|p| p.route_goto_line.as_mut())
    }

    /// Opens the palette in the go-to-line route with a prepared input,
    /// regardless of whether a file is open (tests drive the input directly).
    pub(crate) fn install_palette_goto_line(&mut self, goto: crate::search::GotoLineState) {
        let mut palette = self.new_palette();
        palette.route = crate::command_palette::PaletteRoute::GotoLine;
        palette.route_goto_line = Some(goto);
        palette.opened_in_route = true;
        self.command_palette = Some(palette);
    }
}
