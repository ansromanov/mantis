//! Multiple project tabs, each an independent [`App`], plus the top-level
//! event routing needed to add/close/switch between them.
//!
//! `mantis` is single-root at its core: `App` owns exactly one project's tree
//! and content pane. Rather than exploding `App`'s already-large field set
//! into a nested per-tab structure, `Tabs` treats **one `App` = one tab** and
//! is a thin wrapper around `Vec<App>` — every tab is built, driven, and torn
//! down exactly the way a single-root `mantis` launch already works. `App`
//! itself is untouched aside from one field, `tab_action_request`, that the
//! tab keybindings/command-palette entries set (since a single-root `App` has
//! no way to act on "next tab" itself) and that [`Tabs::dispatch_event`] here
//! checks and clears after every event.

use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, MouseEvent};
use ratatui::layout::Rect;

use crate::app::{App, TabAction};

/// The set of open tabs (project roots) in one `mantis` process.
pub struct Tabs {
    pub apps: Vec<App>,
    pub active: usize,
    /// Path being typed for "open project as new tab"; `Some` while that
    /// inline prompt is open.
    pub new_tab_prompt: Option<String>,
    /// The tab strip's on-screen `Rect` as of the last frame, recorded by
    /// `ui::tabstrip::draw_tabstrip` for mouse hit-testing — the same
    /// record-geometry-at-draw-time pattern `App` uses for `tree_area` etc.
    pub strip_area: Rect,
}

impl Tabs {
    /// Wraps an already-built set of `App`s (one per tab). `active` is
    /// clamped to a valid index.
    pub fn new(apps: Vec<App>, active: usize) -> Self {
        let active = active.min(apps.len().saturating_sub(1));
        Tabs {
            apps,
            active,
            new_tab_prompt: None,
            strip_area: Rect::default(),
        }
    }

    pub fn active_app(&self) -> &App {
        &self.apps[self.active]
    }

    pub fn active_app_mut(&mut self) -> &mut App {
        &mut self.apps[self.active]
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
        Ok(())
    }

    /// Closes the active tab, persisting its session first. No-op when it's
    /// the only tab open — there is always at least one.
    pub fn close_active_tab(&mut self) {
        if self.apps.len() <= 1 {
            return;
        }
        let mut app = self.apps.remove(self.active);
        app.save_session();
        app.plugin_manager.on_quit();
        app.plugin_manager.deactivate_all();
        if self.active >= self.apps.len() {
            self.active = self.apps.len() - 1;
        }
    }

    /// Switches to the next tab, wrapping around. No-op with one tab.
    pub fn next_tab(&mut self) {
        if self.apps.len() > 1 {
            self.active = (self.active + 1) % self.apps.len();
        }
    }

    /// Switches to the previous tab, wrapping around. No-op with one tab.
    pub fn prev_tab(&mut self) {
        if self.apps.len() > 1 {
            self.active = (self.active + self.apps.len() - 1) % self.apps.len();
        }
    }

    /// Saves every tab's own session, then persists the workspace manifest
    /// (open roots + active index) so a future no-args launch can restore it.
    pub fn save_all_and_persist_workspace(&mut self) {
        for app in &mut self.apps {
            app.save_session();
        }
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
            Event::Key(key) => self.active_app_mut().handle_key(key),
            Event::Mouse(m) => self.dispatch_mouse(m),
            _ => return,
        }
        self.apply_pending_tab_action();
    }

    /// Handles a click that landed on the tab strip. Returns `true` when the
    /// event was consumed there (so it must not also reach the active app).
    fn dispatch_strip_mouse(&mut self, m: &MouseEvent) -> bool {
        let strip_area = self.strip_area;
        if self.apps.len() <= 1 || strip_area.height == 0 {
            return false;
        }
        let Some(hit) = crate::ui::tabstrip::hit_test(self, strip_area, m.column, m.row) else {
            return false;
        };
        if matches!(
            m.kind,
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left)
        ) {
            match hit {
                crate::ui::tabstrip::TabHit::Switch(i) => self.active = i,
                crate::ui::tabstrip::TabHit::Close(i) => {
                    self.active = i;
                    self.close_active_tab();
                }
            }
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
        match action {
            TabAction::New => self.new_tab_prompt = Some(String::new()),
            TabAction::Close => self.close_active_tab(),
            TabAction::Next => self.next_tab(),
            TabAction::Prev => self.prev_tab(),
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
