//! Key handling for the fuzzy-picker overlays.
//!
//! Each overlay handler is a thin wrapper around the shared
//! [`handle_list_picker_key`] dispatcher: handle extra keys first, then
//! fall through to the dispatcher and map `Activate`/`Close` to the
//! overlay-specific action.

use std::collections::HashSet;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};

use crate::config::static_keys;
use crate::list_picker::{handle_list_picker_key, OverlayKey};
use crate::theme::Theme;

use super::super::App;

impl App {
    /// Handles keyboard input while the search overlay is open.
    /// Extra keys: Tab toggles file/content mode.
    pub(super) fn handle_search_key(&mut self, key: KeyEvent) {
        // Extra keys first
        if static_keys::is_toggle_modal(&key) {
            if let Some(s) = &mut self.search {
                s.toggle_mode();
            }
            return;
        }
        if let Some(toggle) = static_keys::search_toggle(&key) {
            if let Some(s) = &mut self.search {
                // Only Content mode reads these flags (see refresh_content); ignore
                // the key in Files mode so it doesn't reset the selection for a
                // toggle that would have no visible effect.
                if s.mode == crate::search::SearchMode::Content {
                    match toggle {
                        static_keys::SearchToggle::Regex => s.regex = !s.regex,
                        static_keys::SearchToggle::CaseSensitive => {
                            s.case_sensitive = !s.case_sensitive
                        }
                        static_keys::SearchToggle::WholeWord => s.whole_word = !s.whole_word,
                    }
                    s.refresh_now();
                }
            }
            return;
        }
        let Some(ref mut s) = self.search else { return };
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate => self.activate_search_selection(),
            OverlayKey::Close => {
                self.last_search_query = s.query.clone();
                self.search = None;
            }
            _ => {}
        }
    }

    /// Handles keyboard input while in-file search is active.
    /// Extra keys: n/N/Tab/BackTab/Ctrl-p/Up/Down navigate matches.
    pub(super) fn handle_in_file_search_key(&mut self, key: KeyEvent) {
        // Extra keys first
        if static_keys::is_next_match(&key) {
            self.in_file_search_next();
            return;
        }
        if static_keys::is_prev_match(&key) {
            self.in_file_search_prev();
            return;
        }
        if let Some(toggle) = static_keys::search_toggle(&key) {
            if let Some(s) = &mut self.in_file_search {
                match toggle {
                    static_keys::SearchToggle::Regex => s.regex = !s.regex,
                    static_keys::SearchToggle::CaseSensitive => {
                        s.case_sensitive = !s.case_sensitive
                    }
                    static_keys::SearchToggle::WholeWord => s.whole_word = !s.whole_word,
                }
            }
            self.refresh_in_file_search();
            self.scroll_in_file_search_to_current();
            return;
        }
        let Some(ref mut s) = self.in_file_search else {
            return;
        };
        match handle_list_picker_key(s, &key) {
            OverlayKey::Activate | OverlayKey::Close => {
                self.in_file_search = None;
            }
            OverlayKey::Handled => {
                self.refresh_in_file_search();
                self.scroll_in_file_search_to_current();
            }
            _ => {}
        }
    }

    pub(crate) fn in_file_search_next(&mut self) {
        if let Some(s) = &mut self.in_file_search {
            if !s.matches.is_empty() {
                s.current = (s.current + 1) % s.matches.len();
            }
        }
        self.scroll_in_file_search_to_current();
    }

    pub(crate) fn in_file_search_prev(&mut self) {
        if let Some(s) = &mut self.in_file_search {
            if !s.matches.is_empty() {
                s.current = if s.current == 0 {
                    s.matches.len() - 1
                } else {
                    s.current - 1
                };
            }
        }
        self.scroll_in_file_search_to_current();
    }

    pub(crate) fn scroll_in_file_search_to_current(&mut self) {
        let Some(s) = &self.in_file_search else {
            return;
        };
        let Some(m) = s.matches.get(s.current) else {
            return;
        };
        let display_line = self.physical_to_display(m.line);
        self.scroll_line_into_view(display_line);
        self.mark_content_scrolled();
    }

    /// Re-runs in-file search against the current content source.
    pub(crate) fn refresh_in_file_search(&mut self) {
        let total = self.line_count();
        // Collect before mutably borrowing in_file_search: line_text() takes
        // &self which conflicts with the &mut borrow needed by s.refresh().
        let lines: Vec<String> = (0..total)
            .filter_map(|i| self.line_text(i).map(String::from))
            .collect();
        let Some(s) = &mut self.in_file_search else {
            return;
        };
        s.refresh(total, |i| lines.get(i).cloned());
    }

    /// Handles keyboard input while the git-history overlay is open.
    pub(super) fn handle_history_key(&mut self, key: KeyEvent) {
        let Some(ref mut h) = self.history else {
            return;
        };
        match handle_list_picker_key(h, &key) {
            OverlayKey::Activate => self.show_selected_revision(),
            OverlayKey::Close => self.history = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the repo-wide commit log overlay is open.
    /// Enter enters compare mode for the selected commit; Down at the end of
    /// the list triggers paged loading.
    pub(super) fn handle_repo_log_key(&mut self, key: KeyEvent) {
        let should_load_more = if let Some(ref r) = self.repo_log {
            // 'j' navigates only while the query is empty; otherwise it is a
            // query character and must not trigger paging.
            (matches!(key.code, KeyCode::Down)
                || (matches!(key.code, KeyCode::Char('j')) && r.query.is_empty()))
                && r.selected + 1 >= r.results_len()
        } else {
            false
        };
        if should_load_more {
            if let Some(ref mut r) = self.repo_log {
                r.load_more();
            }
        }
        let Some(ref mut r) = self.repo_log else {
            return;
        };
        match handle_list_picker_key(r, &key) {
            OverlayKey::Activate => self.show_repo_log_compare(),
            OverlayKey::Close => self.repo_log = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the theme picker overlay is open.
    ///
    /// Navigation (j/k/arrows) previews the highlighted theme live behind the
    /// popup; Esc reverts to the original theme that was active before the
    /// picker opened; Enter commits the previewed theme to config.
    pub(super) fn handle_theme_key(&mut self, key: KeyEvent) {
        let (result, selected_name, selected_theme) = {
            let Some(ref mut p) = self.theme_picker else {
                return;
            };
            let result = handle_list_picker_key(p, &key);
            let selected_name = p.selected_name().map(String::from);
            // The picker already parsed every theme once in `discover_all`;
            // reuse that instead of re-reading/re-parsing from disk on
            // every navigation keystroke.
            let selected_theme = p.selected_theme().cloned();
            (result, selected_name, selected_theme)
        };
        match result {
            OverlayKey::Activate => self.apply_selected_theme(),
            OverlayKey::Close => {
                self.theme_picker = None;
                // Revert to the original theme that was active before the picker
                // opened (self.config.theme is never written during preview).
                let config_theme = self.config.theme.clone();
                let theme = config_theme.resolve();
                let requested_name = config_theme
                    .name
                    .clone()
                    .unwrap_or_else(|| "default".to_string());
                // resolve() silently falls back to the default theme when the
                // configured name can't be loaded; report that same fallback
                // name here so plugins/UI aren't told an invalid theme name.
                let name = if Theme::load(&requested_name).is_some() {
                    requested_name
                } else {
                    "default".to_string()
                };
                self.apply_theme(&name, theme);
            }
            OverlayKey::Handled => {
                if let (Some(name), Some(theme)) = (selected_name, selected_theme) {
                    self.apply_theme(&name, theme);
                }
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the recent-files overlay is open.
    pub(super) fn handle_recent_key(&mut self, key: KeyEvent) {
        let Some(ref mut r) = self.recent_files else {
            return;
        };
        match handle_list_picker_key(r, &key) {
            OverlayKey::Activate => self.activate_recent_selection(),
            OverlayKey::Close => self.recent_files = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the plugin manager overlay is open.
    /// Extra keys: Space toggles; j/k navigate (handled by list_picker now).
    pub(super) fn handle_plugin_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.plugin_picker else {
            return;
        };
        // Extra keys first
        if static_keys::is_toggle_selection(&key) {
            self.toggle_plugin_picker_selection();
            return;
        }
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => self.toggle_plugin_picker_selection(),
            OverlayKey::Close => self.plugin_picker = None,
            _ => {}
        }
    }

    /// Handles keyboard input while the inline tree filter is open.
    /// Up/Down/PageUp/PageDown navigate between matches; other keys
    /// (typing, Backspace, Esc, Enter) go through the generic picker.
    pub(super) fn handle_tree_filter_key(&mut self, key: KeyEvent) {
        if self.tree_filter.is_none() {
            return;
        }
        let page = (self.tree_area.height as usize).max(1) as isize;
        if key.code == KeyCode::Up {
            self.move_tree_filter_selection(-1);
            return;
        }
        if key.code == KeyCode::Down {
            self.move_tree_filter_selection(1);
            return;
        }
        if static_keys::is_page_up(&key) {
            self.move_tree_filter_selection(-page);
            return;
        }
        if static_keys::is_page_down(&key) {
            self.move_tree_filter_selection(page);
            return;
        }
        let Some(f) = self.tree_filter.as_mut() else {
            return;
        };
        let is_query_edit = matches!(key.code, KeyCode::Char(_) | KeyCode::Backspace);
        match handle_list_picker_key(f, &key) {
            OverlayKey::Activate => {
                self.tree_filter = None;
                self.activate_selected();
            }
            OverlayKey::Close => {
                self.restore_tree_filter_expansion();
                self.tree_filter = None;
            }
            OverlayKey::Handled => {
                if is_query_edit {
                    self.sync_tree_filter_expansion();
                }
                self.move_tree_filter_selection_to_first_match();
            }
            _ => {}
        }
    }

    /// Auto-expands the ancestor chain of every full-tree match for the
    /// current filter query so matches inside collapsed directories become
    /// navigable, and rebuilds the visible node list to reflect it. Matching
    /// walks the whole tree (not just `self.nodes`, which only contains
    /// currently-visible entries) via a cache built on the filter's first
    /// non-empty query. Restores the pre-filter expansion state instead when
    /// the query becomes empty (e.g. backspacing it all out).
    fn sync_tree_filter_expansion(&mut self) {
        let Some(filter) = self.tree_filter.as_ref() else {
            return;
        };
        if filter.is_empty() {
            self.restore_tree_filter_expansion();
            return;
        }

        if self
            .tree_filter
            .as_ref()
            .is_some_and(|f| f.full_paths_cache.is_none())
        {
            let mut all =
                crate::tree::collect_all_dirs(&self.root, self.show_hidden, self.ignore_gitignore);
            all.extend(crate::tree::collect_all_files(
                &self.root,
                self.show_hidden,
                self.ignore_gitignore,
            ));
            // Pair each path with its lowercased file name up front so the
            // per-keystroke matching below is allocation-free.
            let all: Vec<(PathBuf, String)> = all
                .into_iter()
                .map(|p| {
                    let name = p
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    (p, name)
                })
                .collect();
            if let Some(f) = self.tree_filter.as_mut() {
                f.full_paths_cache = Some(all);
            }
        }

        // Take ownership of the cache first (requires mutable access) so we
        // don't hold an immutable borrow across it.
        let paths = self
            .tree_filter
            .as_mut()
            .and_then(|f| f.full_paths_cache.take())
            .unwrap_or_default();

        let Some(filter) = self.tree_filter.as_ref() else {
            return;
        };

        let mut to_expand: HashSet<PathBuf> = HashSet::new();
        for (path, name) in &paths {
            // `name` is already lowercased in the cache; `matches_name` is
            // case-insensitive so matching against a lowercased name is fine.
            if !filter.matches_name(name) {
                continue;
            }
            let mut ancestor = path.parent();
            while let Some(p) = ancestor {
                if p == self.root {
                    break;
                }
                // An earlier match already recorded this ancestor, and that
                // walk continued to the root, so the rest of this chain is
                // recorded too.
                if to_expand.contains(p) {
                    break;
                }
                if !self.expanded.contains(p) {
                    to_expand.insert(p.to_path_buf());
                }
                ancestor = p.parent();
            }
        }

        if let Some(f) = self.tree_filter.as_mut() {
            f.full_paths_cache = Some(paths);
        }

        if !to_expand.is_empty() {
            if self
                .tree_filter
                .as_ref()
                .is_some_and(|f| f.saved_expanded.is_none())
            {
                let snapshot = self.expanded.clone();
                if let Some(f) = self.tree_filter.as_mut() {
                    f.saved_expanded = Some(snapshot);
                }
            }
            self.expanded.extend(to_expand);
            self.rebuild(false);
        }
    }

    /// Restores `expanded` to the snapshot taken before the tree filter
    /// auto-expanded any directories, if one was taken. No-op when the
    /// filter never triggered an auto-expansion (e.g. all matches were
    /// already visible), or when the snapshot matches the current state.
    fn restore_tree_filter_expansion(&mut self) {
        let saved = self
            .tree_filter
            .as_mut()
            .and_then(|f| f.saved_expanded.take());
        if let Some(saved) = saved {
            if saved != self.expanded {
                self.expanded = saved;
                self.rebuild(false);
            }
        }
    }

    /// Move the tree-filter selection by `delta` rows within the filtered
    /// match set (`tree_visible_indices`), clamping to its bounds. Does
    /// nothing when no filter set is recorded or the set is empty.
    fn move_tree_filter_selection(&mut self, delta: isize) {
        let visible: Vec<usize> = match self.tree_visible_indices.as_ref() {
            Some(v) if !v.is_empty() => v.clone(),
            _ => return,
        };
        let cur_pos = visible
            .iter()
            .position(|&i| i == self.tree_selected)
            .unwrap_or(0);
        let new_pos = (cur_pos as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
        if let Some(&node_idx) = visible.get(new_pos) {
            self.tree_selected = node_idx;
            self.scroll_tree_into_view();
        }
    }

    /// Moves `tree_selected` to the first visible match index when the inline
    /// tree filter is active. If the query is empty or no node matches, the
    /// selection stays at index 0 (the root).
    fn move_tree_filter_selection_to_first_match(&mut self) {
        let Some(ref filter) = self.tree_filter else {
            return;
        };
        if filter.is_empty() {
            self.tree_selected = 0;
            self.scroll_tree_into_view();
            return;
        }
        let first_match = self.nodes.iter().position(|n| filter.matches_name(&n.name));
        self.tree_selected = first_match.unwrap_or(0);
        self.scroll_tree_into_view();
    }

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
                }
            }
            Some(OverlayKey::Close) => {
                if let Some(ref mut p) = self.command_palette {
                    let in_routed_mode = p.route != crate::command_palette::PaletteRoute::Commands;
                    if in_routed_mode {
                        p.route = crate::command_palette::PaletteRoute::Commands;
                        p.route_search = None;
                        p.route_goto_line = None;
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
            crate::command_palette::PaletteRoute::Commands => {}
        }
    }

    /// Handles keyboard input while the revision picker is open.
    /// Uses the shared list-picker dispatcher for navigation, plus Enter to
    /// select (falling back to the typed query as a raw revspec when the
    /// filtered list is empty) and Esc to close. Left/Right arrows switch
    /// between tabs when the query is empty.
    pub(super) fn handle_revision_key(&mut self, key: KeyEvent) {
        let Some(ref mut p) = self.revision_picker else {
            return;
        };
        // Left/Right switch tabs when the query is empty.
        if p.query.is_empty() {
            match key.code {
                KeyCode::Left => {
                    p.prev_tab();
                    return;
                }
                KeyCode::Right => {
                    p.next_tab();
                    return;
                }
                _ => {}
            }
        }
        match handle_list_picker_key(p, &key) {
            OverlayKey::Activate => {
                let rev = if p.results_len() > 0 && p.selected < p.results_len() {
                    p.selected_rev().map(|r| r.to_string())
                } else if !p.query.is_empty() {
                    Some(p.query.clone())
                } else {
                    None
                };
                self.revision_picker = None;
                if let Some(rev) = rev {
                    self.enter_compare_mode(rev);
                }
            }
            OverlayKey::Close => {
                self.revision_picker = None;
            }
            _ => {}
        }
    }

    /// Handles keyboard input while the go-to-line dialog is open.
    /// Extra keys: filters out the open binding so it is not appended.
    pub(super) fn handle_goto_line_key(&mut self, key: KeyEvent) {
        // Filter out the open binding key before passing to the shared dispatcher.
        if let KeyCode::Char(_) = &key.code {
            if crate::config::pressed(&self.config.keys.goto_line, &key) {
                return;
            }
        }
        let Some(ref mut g) = self.goto_line else {
            return;
        };
        match handle_list_picker_key(g, &key) {
            OverlayKey::Activate => {
                let query = self.goto_line.as_ref().map(|g| g.query.clone());
                self.goto_line = None;
                if let Some(q) = query {
                    self.goto_line_from_query(&q);
                }
            }
            OverlayKey::Close => {
                self.goto_line = None;
            }
            _ => {}
        }
    }
    pub(super) fn handle_bug_report_key(&mut self, key: KeyEvent) {
        use crossterm::event::KeyModifiers;

        let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let is_alt = key.modifiers.contains(KeyModifiers::ALT);

        // Submit/Save: Ctrl+S or Ctrl+Enter
        if is_ctrl
            && (key.code == KeyCode::Char('s')
                || key.code == KeyCode::Char('S')
                || key.code == KeyCode::Enter)
        {
            self.save_bug_report();
            return;
        }

        use crate::search::BugReportFocus;

        match key.code {
            KeyCode::Esc => {
                self.bug_report = None;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(ref mut state) = self.bug_report {
                    state.toggle_focus();
                }
            }
            KeyCode::Enter => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.focus = BugReportFocus::Description,
                        BugReportFocus::Description => state.insert_newline(),
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_backspace(),
                        BugReportFocus::Description => state.backspace(),
                    }
                }
            }
            KeyCode::Delete => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_delete(),
                        BugReportFocus::Description => state.delete(),
                    }
                }
            }
            KeyCode::Left => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_left(),
                        BugReportFocus::Description => state.move_left(),
                    }
                }
            }
            KeyCode::Right => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_right(),
                        BugReportFocus::Description => state.move_right(),
                    }
                }
            }
            KeyCode::Up => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => {}
                        BugReportFocus::Description if state.cursor_row == 0 => {
                            state.focus = BugReportFocus::Title;
                        }
                        BugReportFocus::Description => state.move_up(),
                    }
                }
            }
            KeyCode::Down => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.focus = BugReportFocus::Description,
                        BugReportFocus::Description => state.move_down(),
                    }
                }
            }
            KeyCode::Home => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_home(),
                        BugReportFocus::Description => state.move_home(),
                    }
                }
            }
            KeyCode::End => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_move_end(),
                        BugReportFocus::Description => state.move_end(),
                    }
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut state) = self.bug_report {
                    state.preview_scroll.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut state) = self.bug_report {
                    let body_text = state.text.join("\n");
                    let report_md = if body_text.trim().is_empty() {
                        state.diagnostics_markdown.clone()
                    } else {
                        format!(
                            "## {}\n\n## bug report body\n\n{}\n\n{}",
                            state.title, body_text, state.diagnostics_markdown
                        )
                    };
                    let lines_count = report_md.lines().count();
                    let height = self.bug_report_preview_area.height.max(1) as usize;
                    let max_scroll = lines_count.saturating_sub(height);
                    state.preview_scroll.scroll_down(10, max_scroll);
                }
            }
            KeyCode::Char(c) if !is_ctrl && !is_alt => {
                if let Some(ref mut state) = self.bug_report {
                    match state.focus {
                        BugReportFocus::Title => state.title_insert_char(c),
                        BugReportFocus::Description => state.insert_char(c),
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "overlay_test.rs"]
mod tests;
