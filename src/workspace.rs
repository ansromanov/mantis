//! Multiple project tabs, each an independent [`App`], plus the top-level
//! event routing needed to add/close/switch between them.
//!
//! `mantis` is single-root at its core: `App` owns exactly one project's tree
//! and content pane. Rather than exploding `App`'s already-large field set
//! into a nested per-tab structure, `Tabs` treats **one `App` = one tab** and
//! is a thin wrapper around `Vec<App>` — every tab is built, driven, and torn
//! down exactly the way a single-root `mantis` launch already works. `Tabs`
//! tracks the first visible tab so the active tab remains in view and routes
//! mouse input for the tab-strip scroll affordances. `App`
//! itself is untouched aside from one field, `tab_action_request`, that the
//! tab keybindings/command-palette entries set (since a single-root `App` has
//! no way to act on "next tab" itself) and that [`Tabs::dispatch_event`] here
//! checks and clears after every event.

use std::collections::VecDeque;
use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use crate::app::{App, TabAction};

/// The set of open tabs (project roots) in one `mantis` process.
pub struct Tabs {
    pub apps: Vec<App>,
    pub active: usize,
    /// Index of the first visible tab in the tab strip (horizontal scroll offset).
    pub first_visible: usize,
    /// Path being typed for "open project as new tab"; `Some` while that
    /// inline prompt is open.
    pub new_tab_prompt: Option<String>,
    /// The tab strip's on-screen `Rect` as of the last frame, recorded by
    /// `ui::tabstrip::draw_tabstrip` for mouse hit-testing — the same
    /// record-geometry-at-draw-time pattern `App` uses for `tree_area` etc.
    pub strip_area: Rect,
    /// Recently closed roots, newest last, for the reopen-tab action.
    pub closed_tabs: VecDeque<PathBuf>,
    /// Workspace-level fuzzy picker for switching among open tabs.
    pub tab_picker: Option<crate::search::TabPicker>,
    /// The popup rectangle from the last workspace-picker render.
    pub tab_picker_area: Rect,
    /// Tab pressed on the strip, used to finish a drag reorder on release.
    pub drag_tab: Option<usize>,
}

impl Tabs {
    /// Wraps an already-built set of `App`s (one per tab). `active` is
    /// clamped to a valid index and scrolled into view.
    pub fn new(apps: Vec<App>, active: usize) -> Self {
        let active = active.min(apps.len().saturating_sub(1));
        let mut tabs = Tabs {
            apps,
            active,
            first_visible: 0,
            new_tab_prompt: None,
            strip_area: Rect::default(),
            closed_tabs: VecDeque::new(),
            tab_picker: None,
            tab_picker_area: Rect::default(),
            drag_tab: None,
        };
        tabs.ensure_active_visible();
        tabs
    }

    pub fn active_app(&self) -> &App {
        &self.apps[self.active]
    }

    pub fn active_app_mut(&mut self) -> &mut App {
        &mut self.apps[self.active]
    }

    /// Ensures that the active tab is visible, using the last drawn strip width.
    pub fn ensure_active_visible(&mut self) {
        let width = if self.strip_area.width > 0 {
            self.strip_area.width
        } else {
            80
        };
        self.ensure_active_visible_for_width(width);
    }

    /// Ensures that the active tab is visible for a specific strip width.
    pub fn ensure_active_visible_for_width(&mut self, width: u16) {
        if self.apps.is_empty() {
            self.active = 0;
            self.first_visible = 0;
            return;
        }
        self.active = self.active.min(self.apps.len() - 1);
        self.first_visible = self.first_visible.min(self.apps.len() - 1);
        if self.active < self.first_visible {
            self.first_visible = self.active;
        }
        while self.first_visible < self.active
            && !crate::ui::tabstrip::is_tab_visible(self, self.first_visible, self.active, width)
        {
            self.first_visible += 1;
        }
    }

    /// Scrolls the tab strip one tab to the left, if possible.
    pub fn scroll_strip_left(&mut self) {
        self.first_visible = self.first_visible.saturating_sub(1);
    }

    /// Scrolls the tab strip one tab to the right, if possible.
    pub fn scroll_strip_right(&mut self) {
        if self.first_visible + 1 < self.apps.len() {
            self.first_visible += 1;
        }
    }

    /// Selects a tab, clamping the requested index to the last open tab.
    pub fn set_active(&mut self, index: usize) {
        if !self.apps.is_empty() {
            self.active = index.min(self.apps.len() - 1);
            self.ensure_active_visible();
        }
    }

    /// Builds a new `App` for `root` (loading its own `mantis.toml`, exactly
    /// as a top-level launch would) and makes it the active tab.
    pub fn open_tab(&mut self, root: PathBuf) -> anyhow::Result<()> {
        let (cfg, cfg_path, cfg_error) = crate::config::load(&root);
        let mut app = App::new(root, cfg, cfg_path, cfg_error)?;
        app.watch_root();
        app.install_config_watcher();
        self.apps.push(app);
        self.active = self.apps.len() - 1;
        self.ensure_active_visible();
        Ok(())
    }

    /// Closes the active tab, persisting its session first. No-op when it's
    /// the only tab open — there is always at least one.
    pub fn close_active_tab(&mut self) {
        if self.apps.len() <= 1 {
            return;
        }
        let mut app = self.apps.remove(self.active);
        let root = app.root.clone();
        app.save_session();
        app.plugin_manager.on_quit();
        app.plugin_manager.deactivate_all();
        self.closed_tabs.push_back(root);
        while self.closed_tabs.len() > 10 {
            self.closed_tabs.pop_front();
        }
        if self.active >= self.apps.len() {
            self.active = self.apps.len() - 1;
        }
        self.first_visible = self.first_visible.min(self.apps.len() - 1);
        self.ensure_active_visible();
    }

    fn close_tab_at(&mut self, index: usize) {
        if self.apps.len() <= 1 || index >= self.apps.len() {
            return;
        }
        let mut app = self.apps.remove(index);
        app.save_session();
        app.plugin_manager.on_quit();
        app.plugin_manager.deactivate_all();
        if self.active > index {
            self.active -= 1;
        }
        self.active = self.active.min(self.apps.len().saturating_sub(1));
    }

    fn close_tabs_to_right(&mut self, index: usize) {
        while self.apps.len() > index.saturating_add(1) {
            self.close_tab_at(self.apps.len() - 1);
        }
        self.active = index.min(self.apps.len().saturating_sub(1));
    }

    fn close_other_tabs(&mut self, index: usize) {
        if index >= self.apps.len() {
            return;
        }
        let mut kept = None;
        for (i, mut app) in self.apps.drain(..).enumerate() {
            if i == index {
                kept = Some(app);
            } else {
                app.save_session();
                app.plugin_manager.on_quit();
                app.plugin_manager.deactivate_all();
            }
        }
        if let Some(app) = kept {
            self.apps.push(app);
            self.active = 0;
        }
    }

    /// Switches to the next tab, wrapping around. No-op with one tab.
    pub fn next_tab(&mut self) {
        if self.apps.len() > 1 {
            self.active = (self.active + 1) % self.apps.len();
            self.ensure_active_visible();
        }
    }

    /// Switches to the previous tab, wrapping around. No-op with one tab.
    pub fn prev_tab(&mut self) {
        if self.apps.len() > 1 {
            self.active = (self.active + self.apps.len() - 1) % self.apps.len();
            self.ensure_active_visible();
        }
    }

    /// Selects a tab, clamping the requested index to the last open tab.
    pub fn select_tab(&mut self, index: usize) {
        if !self.apps.is_empty() {
            self.set_active(index);
        }
    }

    /// Moves a tab to a destination index and keeps it active.
    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.apps.len() || to >= self.apps.len() || from == to {
            return;
        }
        let app = self.apps.remove(from);
        self.apps.insert(to, app);
        self.active = to;
        self.ensure_active_visible();
        self.persist_manifest();
    }

    /// Moves the active tab one position, stopping at the strip's ends.
    pub fn move_active_tab(&mut self, delta: isize) {
        if self.apps.is_empty() {
            return;
        }
        let to = self
            .active
            .saturating_add_signed(delta)
            .min(self.apps.len() - 1);
        self.move_tab(self.active, to);
    }

    /// Reopens the most recently closed root, restoring its saved app session.
    pub fn reopen_closed_tab(&mut self) {
        let Some(root) = self.closed_tabs.pop_back() else {
            return;
        };
        if let Err(err) = self.open_tab(root.clone()) {
            self.active_app_mut().set_status(format!(
                "couldn't reopen '{}' as a tab: {err}",
                root.display()
            ));
        }
    }

    /// Saves every tab's own session, then persists the workspace manifest
    /// (open roots + active index) so a future no-args launch can restore it.
    pub fn save_all_and_persist_workspace(&mut self) {
        for app in &mut self.apps {
            app.save_session();
        }
        self.persist_manifest();
    }

    fn persist_manifest(&self) {
        crate::session::save_workspace(&crate::session::WorkspaceState {
            roots: self.apps.iter().map(|a| a.root.clone()).collect(),
            active: self.active,
        });
    }

    /// Handles a key event while the "open project as new tab" prompt is
    /// open: Enter resolves the typed path and opens it, Esc cancels.
    /// A path that fails to resolve leaves the prompt open with an inline
    /// error message so the user can correct it.
    fn handle_new_tab_prompt_key(&mut self, key: KeyEvent) {
        let Some(query) = self.new_tab_prompt.as_mut() else {
            return;
        };
        match key.code {
            KeyCode::Esc => self.new_tab_prompt = None,
            KeyCode::Enter => {
                let typed = query.trim().to_string();
                match resolve_tab_path(&typed) {
                    Ok(root) => {
                        self.new_tab_prompt = None;
                        if let Err(err) = self.open_tab(root) {
                            self.active_app_mut()
                                .set_status(format!("couldn't open '{typed}' as a tab: {err}"));
                        }
                    }
                    Err(reason) => {
                        if let Some(q) = self.new_tab_prompt.as_mut() {
                            *q = format!("{typed} — {reason}");
                        }
                    }
                }
            }
            KeyCode::Backspace => {
                query.pop();
            }
            KeyCode::Char(c) => query.push(c),
            _ => {}
        }
    }

    /// Dispatches one terminal event: a mouse click on the tab strip is
    /// handled here first (switch/close tab); otherwise routes to the
    /// new-tab prompt when it's open, or to the active `App`, then checks
    /// whether that `App` requested a tab-lifecycle action (`new_tab`/
    /// `close_tab`/`next_tab`/`prev_tab`, set via keybinding or the command
    /// palette) and acts on it.
    pub fn dispatch_event(&mut self, event: Event) {
        if self.tab_picker.is_some() {
            match event {
                Event::Key(key) => self.handle_tab_picker_key(key),
                Event::Mouse(mouse)
                    if matches!(
                        mouse.kind,
                        MouseEventKind::Down(crossterm::event::MouseButton::Left)
                    ) && !crate::app::rect_contains(
                        self.tab_picker_area,
                        mouse.column,
                        mouse.row,
                    ) =>
                {
                    self.tab_picker = None;
                }
                _ => {}
            }
            return;
        }
        if let Event::Mouse(m) = &event {
            if self.dispatch_strip_mouse(m) {
                return;
            }
        }
        match event {
            Event::Key(key) if self.new_tab_prompt.is_some() => {
                self.handle_new_tab_prompt_key(key);
                return;
            }
            Event::Key(key) if self.select_tab_key(key) => return,
            Event::Key(key) => self.active_app_mut().handle_key(key),
            Event::Mouse(m) => self.dispatch_mouse(m),
            _ => return,
        }
        self.apply_pending_tab_action();
    }

    fn select_tab_key(&mut self, key: KeyEvent) -> bool {
        if !crate::config::pressed(&self.active_app().keys().select_tab, &key) {
            return false;
        }
        let KeyCode::Char(digit) = key.code else {
            return false;
        };
        if !digit.is_ascii_digit() {
            return false;
        }
        let index = match digit {
            '0' => self.apps.len().saturating_sub(1),
            '1'..='9' => digit as usize - '0' as usize - 1,
            _ => return false,
        };
        self.apply_tab_action(TabAction::Select(index));
        true
    }

    fn handle_tab_picker_key(&mut self, key: KeyEvent) {
        use crate::list_picker::{handle_list_picker_key, OverlayKey};
        let Some(picker) = self.tab_picker.as_mut() else {
            return;
        };
        match handle_list_picker_key(picker, &key) {
            OverlayKey::Close => self.tab_picker = None,
            OverlayKey::Activate => {
                let selected = picker.selected_tab();
                self.tab_picker = None;
                if let Some(index) = selected {
                    self.select_tab(index);
                }
            }
            OverlayKey::Handled | OverlayKey::Pass => {}
        }
    }

    /// Handles a click that landed on the tab strip. Returns `true` when the
    /// event was consumed there (so it must not also reach the active app).
    fn dispatch_strip_mouse(&mut self, m: &MouseEvent) -> bool {
        let strip_area = self.strip_area;
        if self.apps.len() <= 1 || strip_area.height == 0 {
            return false;
        }
        if m.row >= strip_area.y
            && m.row < strip_area.y.saturating_add(strip_area.height)
            && m.column >= strip_area.x
            && m.column < strip_area.x.saturating_add(strip_area.width)
        {
            match m.kind {
                MouseEventKind::ScrollUp => {
                    self.scroll_strip_left();
                    return true;
                }
                MouseEventKind::ScrollDown => {
                    self.scroll_strip_right();
                    return true;
                }
                _ => {}
            }
        }
        let Some(hit) = crate::ui::tabstrip::hit_test(self, strip_area, m.column, m.row) else {
            if matches!(
                m.kind,
                MouseEventKind::Up(crossterm::event::MouseButton::Left)
            ) {
                self.drag_tab = None;
            }
            return false;
        };
        match m.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => match hit {
                crate::ui::tabstrip::TabHit::Switch(i) => {
                    self.set_active(i);
                    self.drag_tab = Some(i);
                }
                crate::ui::tabstrip::TabHit::Close(i) => {
                    self.drag_tab = None;
                    self.set_active(i);
                    self.close_active_tab();
                }
                crate::ui::tabstrip::TabHit::ScrollLeft => self.scroll_strip_left(),
                crate::ui::tabstrip::TabHit::ScrollRight => self.scroll_strip_right(),
            },
            MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                if let Some(from) = self.drag_tab.take() {
                    let to = match hit {
                        crate::ui::tabstrip::TabHit::Switch(i)
                        | crate::ui::tabstrip::TabHit::Close(i) => Some(i),
                        crate::ui::tabstrip::TabHit::ScrollLeft
                        | crate::ui::tabstrip::TabHit::ScrollRight => None,
                    };
                    if let Some(to) = to {
                        self.move_tab(from, to);
                    }
                }
            }
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Right) => {
                let i = match hit {
                    crate::ui::tabstrip::TabHit::Switch(i)
                    | crate::ui::tabstrip::TabHit::Close(i) => i,
                };
                self.active = i;
                if let Some(app) = self.apps.get_mut(i) {
                    app.open_tab_context_menu(app.root.clone(), i, (m.column, m.row));
                }
            }
            _ => {}
        }
        true
    }

    fn dispatch_mouse(&mut self, m: MouseEvent) {
        self.active_app_mut().handle_mouse(m);
    }

    fn apply_pending_tab_action(&mut self) {
        let Some(action) = self.active_app_mut().tab_action_request.take() else {
            return;
        };
        self.apply_tab_action(action);
    }

    fn apply_tab_action(&mut self, action: TabAction) {
        match action {
            TabAction::New => self.new_tab_prompt = Some(String::new()),
            TabAction::Open(path) => {
                if let Err(error) = self.open_tab(path) {
                    self.active_app_mut()
                        .set_status(format!("cannot open tab: {error}"));
                }
            }
            TabAction::CloseOthers(index) => self.close_other_tabs(index),
            TabAction::CloseToRight(index) => self.close_tabs_to_right(index),
            TabAction::Close => self.close_active_tab(),
            TabAction::Next => self.next_tab(),
            TabAction::Prev => self.prev_tab(),
            TabAction::Select(index) => self.select_tab(index),
            TabAction::MovePrev => self.move_active_tab(-1),
            TabAction::MoveNext => self.move_active_tab(1),
            TabAction::Picker => {
                self.tab_picker = Some(crate::search::TabPicker::new(&self.apps));
            }
            TabAction::Reopen => self.reopen_closed_tab(),
        }
    }
}

/// Resolves a typed "new tab" path: expands a leading `~`, resolves relative
/// to the current directory, and requires the result to be an existing
/// directory (matching what a CLI path argument accepts).
fn resolve_tab_path(typed: &str) -> Result<PathBuf, &'static str> {
    if typed.is_empty() {
        return Err("enter a directory path");
    }
    let expanded = if let Some(rest) = typed.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(rest))
            .unwrap_or_else(|| PathBuf::from(typed))
    } else {
        PathBuf::from(typed)
    };
    let canonical = expanded.canonicalize().map_err(|_| "no such path")?;
    if !canonical.is_dir() {
        return Err("not a directory");
    }
    Ok(canonical)
}

#[cfg(test)]
#[path = "workspace_test.rs"]
mod tests;
