use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Instant;

use notify::RecommendedWatcher;

use ratatui::layout::Rect;

use crate::config::{self, Config, Keymap};
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::search::{HistoryState, SearchState, ThemePicker};
use crate::selection::TextSelection;
use crate::theme::Theme;
use crate::tree::{build_visible, TreeNode};

mod file_ops;
mod key_handlers;
mod mouse_handlers;
mod navigation;

pub enum Focus {
    Tree,
    Content,
}

pub struct App {
    pub root: PathBuf,
    pub nodes: Vec<TreeNode>,
    pub expanded: HashSet<PathBuf>,
    pub tree_selected: usize,
    pub content: Vec<String>,
    pub highlighted: Vec<Vec<(ratatui::style::Style, String)>>,
    pub markdown_lines: Vec<Vec<(ratatui::style::Style, String)>>,
    pub is_markdown: bool,
    pub show_raw_markdown: bool,
    pub content_scroll: usize,
    pub content_hscroll: usize,
    pub word_wrap: bool,
    pub current_file: Option<PathBuf>,
    pub is_diff: bool,
    pub content_title: Option<String>,
    pub focus: Focus,
    pub search: Option<SearchState>,
    pub history: Option<HistoryState>,
    pub theme_picker: Option<ThemePicker>,
    pub show_hidden: bool,
    pub ignore_gitignore: bool,
    pub tree_width: u16,
    pub show_help: bool,
    pub should_quit: bool,
    pub theme: Theme,
    pub git_status_enabled: bool,
    pub git_show_deleted: bool,
    pub git_status_map: HashMap<PathBuf, GitStatus>,
    pub git_mode: bool,
    pub git_mode_flat: bool,
    keys: Keymap,
    config: Config,
    config_path: Option<std::path::PathBuf>,
    // Geometry captured during the last render, used to map mouse events.
    pub tree_area: Rect,
    pub tree_offset: usize,
    pub content_area: Rect,
    pub search_area: Rect,
    pub search_offset: usize,
    pub history_area: Rect,
    pub history_offset: usize,
    pub theme_area: Rect,
    pub theme_offset: usize,
    // Time and result index of the last search-result click, for double-click.
    last_click: Option<(Instant, usize)>,
    highlighter: Highlighter,
    last_refresh: Instant,
    file_watcher: Option<RecommendedWatcher>,
    file_watch_rx: Option<Receiver<notify::Result<notify::Event>>>,
    file_watch_path: Option<PathBuf>,
    pub selection: Option<TextSelection>,
    drag_start: Option<(usize, usize)>,
}

impl App {
    pub fn new(
        root: PathBuf,
        cfg: Config,
        config_path: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        let expanded = HashSet::new();
        // git_mode requires status data even if git_status is disabled in config.
        let git_status_enabled = cfg.git_status || cfg.git_mode;
        let git_show_deleted = cfg.git_show_deleted;
        let git_status_map = if git_status_enabled {
            crate::git::repo_status(&root, cfg.ignore_gitignore)
        } else {
            HashMap::new()
        };
        let deleted = deleted_set(&git_status_map, git_show_deleted);
        let nodes = build_visible(
            &root,
            &expanded,
            cfg.show_hidden,
            cfg.ignore_gitignore,
            &deleted,
        );
        let theme = cfg.theme.resolve();
        let saved_config = cfg.clone();
        let highlighter = Highlighter::new(&theme.syntax);
        let mut app = App {
            root,
            nodes,
            expanded,
            tree_selected: 0,
            content: Vec::new(),
            highlighted: Vec::new(),
            markdown_lines: Vec::new(),
            is_markdown: false,
            show_raw_markdown: false,
            content_scroll: 0,
            content_hscroll: 0,
            word_wrap: cfg.word_wrap,
            current_file: None,
            is_diff: false,
            content_title: None,
            focus: Focus::Tree,
            search: None,
            history: None,
            theme_picker: None,
            show_hidden: cfg.show_hidden,
            ignore_gitignore: cfg.ignore_gitignore,
            tree_width: cfg.tree_width,
            show_help: false,
            should_quit: false,
            theme,
            git_status_enabled,
            git_show_deleted,
            git_status_map,
            git_mode: cfg.git_mode,
            git_mode_flat: cfg.git_mode_flat,
            keys: cfg.keys,
            config: saved_config,
            config_path,
            tree_area: Rect::default(),
            tree_offset: 0,
            content_area: Rect::default(),
            search_area: Rect::default(),
            search_offset: 0,
            history_area: Rect::default(),
            history_offset: 0,
            theme_area: Rect::default(),
            theme_offset: 0,
            last_click: None,
            highlighter,
            last_refresh: Instant::now(),
            file_watcher: None,
            file_watch_rx: None,
            file_watch_path: None,
            selection: None,
            drag_start: None,
        };
        if app.git_mode {
            app.expand_git_dirs();
            app.rebuild();
        }
        app.try_open_selected();
        Ok(app)
    }

    fn save_config(&self) {
        if let Some(path) = &self.config_path {
            config::save(&self.config, path);
        }
    }

    pub fn reload(&mut self) {
        self.last_refresh = Instant::now();
        if self.git_status_enabled {
            self.git_status_map = crate::git::repo_status(&self.root, self.ignore_gitignore);
        }
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let ignore_gitignore = self.ignore_gitignore;
        if let Some(s) = &mut self.search {
            s.reload_files(&root, show_hidden, ignore_gitignore);
        }
        self.rebuild();
        self.reload_content();
    }

    pub fn tick(&mut self) {
        if self.drain_file_watch() {
            self.reload_content();
        }
        if self.last_refresh.elapsed().as_secs() >= 30 {
            self.reload();
        }
    }
}

/// Builds the set of absolute paths that should appear as ghost (deleted) nodes
/// in the tree. Only files that are absent from the working tree are included.
fn deleted_set(map: &HashMap<PathBuf, GitStatus>, enabled: bool) -> HashSet<PathBuf> {
    if !enabled {
        return HashSet::new();
    }
    map.iter()
        .filter(|(path, &status)| status == GitStatus::Deleted && !path.exists())
        .map(|(path, _)| path.clone())
        .collect()
}

fn rect_contains(area: Rect, col: u16, row: u16) -> bool {
    col >= area.x
        && col < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

/// Colors a unified-diff line by its leading marker.
fn diff_line_style(line: &str, theme: &Theme) -> ratatui::style::Style {
    use ratatui::style::{Modifier, Style};
    if line.starts_with("@@") {
        Style::default().fg(theme.accent)
    } else if line.starts_with("+++") || line.starts_with("---") {
        Style::default().fg(theme.dim).add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(theme.diff_add)
    } else if line.starts_with('-') {
        Style::default().fg(theme.diff_del)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(theme.dim)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::config::Config;
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Creates a temp directory tree:
    ///   sub/ (with c.txt), a.txt, b.txt, long.txt (50 lines)
    fn temp_tree() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_app_test_{}_{n}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), "line1\nline2\n").unwrap();
        fs::write(dir.join("b.txt"), "hello\n").unwrap();
        fs::write(dir.join("sub").join("c.txt"), "nested\n").unwrap();
        let long: String = (1..=50).map(|i| format!("line {i}\n")).collect();
        fs::write(dir.join("long.txt"), long).unwrap();
        dir.canonicalize().unwrap()
    }

    fn app_for(root: &std::path::Path) -> App {
        App::new(root.to_path_buf(), Config::default(), None).unwrap()
    }

    /// A temp git repo with one committed file plus an uncommitted change.
    fn temp_git_tree() -> PathBuf {
        use std::process::Command;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_app_git_{}_{n}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q"]);
        fs::write(dir.join("tracked.txt"), "one\n").unwrap();
        git(&["add", "tracked.txt"]);
        git(&["commit", "-q", "-m", "add tracked"]);
        fs::write(dir.join("tracked.txt"), "one\ntwo\n").unwrap();
        dir.canonicalize().unwrap()
    }

    #[test]
    fn file_history_opens_picker_and_shows_diff() {
        let root = temp_git_tree();
        let mut app = app_for(&root);
        app.open_file(&root.join("tracked.txt"));

        // H opens the history picker.
        app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
        assert!(app.history.is_some());
        assert!(!app.history.as_ref().unwrap().commits.is_empty());

        // Enter loads the diff into the content panel.
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));
        assert!(app.history.is_none());
        assert!(app.is_diff);
        assert!(app.content_title.is_some());
        assert!(app.content.iter().any(|l| l.starts_with("+two")));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn open_and_reveal_selects_file_in_tree() {
        let root = temp_tree();
        let mut app = app_for(&root);
        // Reveal a file nested inside a collapsed subdirectory.
        let nested = root.join("sub").join("c.txt");
        app.open_and_reveal(&nested);

        assert_eq!(app.current_file.as_deref(), Some(nested.as_path()));
        assert!(matches!(app.focus, Focus::Content));
        // The parent dir is expanded and the file node is selected.
        assert!(app.expanded.contains(&root.join("sub")));
        assert_eq!(
            app.nodes.get(app.tree_selected).map(|n| n.path.clone()),
            Some(nested)
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn file_history_noop_without_git_history() {
        let root = temp_tree(); // not a git repo
        let mut app = app_for(&root);
        app.open_file(&root.join("a.txt"));
        app.handle_key(KeyEvent::new(KeyCode::Char('H'), KeyModifiers::empty()));
        assert!(app.history.is_none());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn theme_picker_applies_preset() {
        let root = temp_tree();
        let mut app = app_for(&root);
        assert_eq!(app.theme.accent, crate::theme::Theme::default().accent);

        // `t` opens the picker.
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
        assert!(app.theme_picker.is_some());

        // Filter to "monokai" and apply it.
        for c in "monokai".chars() {
            app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::empty()));
        }
        app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::empty()));

        assert!(app.theme_picker.is_none());
        assert_eq!(
            app.theme.accent,
            crate::theme::Theme::preset("monokai").unwrap().accent
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn theme_picker_esc_cancels() {
        let root = temp_tree();
        let mut app = app_for(&root);
        let before = app.theme.accent;
        app.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::empty()));
        app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::empty()));
        assert!(app.theme_picker.is_none());
        assert_eq!(app.theme.accent, before); // unchanged
        fs::remove_dir_all(&root).ok();
    }

    fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn click(col: u16, row: u16) -> MouseEvent {
        mouse(MouseEventKind::Down(MouseButton::Left), col, row)
    }

    fn full_rect() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 20,
        }
    }

    #[test]
    fn rect_contains_checks_bounds() {
        let r = Rect {
            x: 2,
            y: 3,
            width: 4,
            height: 5,
        };
        assert!(rect_contains(r, 2, 3)); // top-left corner
        assert!(rect_contains(r, 5, 7)); // inside, near far corner
        assert!(!rect_contains(r, 6, 3)); // column == x + width
        assert!(!rect_contains(r, 2, 8)); // row == y + height
        assert!(!rect_contains(r, 1, 3)); // left of area
        assert!(!rect_contains(r, 2, 2)); // above area
    }

    #[test]
    fn left_click_in_tree_opens_file() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;
        app.focus = Focus::Content;

        let idx = app.nodes.iter().position(|n| !n.is_dir).unwrap();
        let path = app.nodes[idx].path.clone();
        app.handle_mouse(click(1, idx as u16));

        assert_eq!(app.tree_selected, idx);
        assert_eq!(app.current_file.as_deref(), Some(path.as_path()));
        assert!(matches!(app.focus, Focus::Tree));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn left_click_on_dir_toggles_expand() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;

        let dir_idx = app.nodes.iter().position(|n| n.is_dir).unwrap();
        let dir_path = app.nodes[dir_idx].path.clone();
        let before = app.nodes.len();

        app.handle_mouse(click(1, dir_idx as u16));
        assert!(app.expanded.contains(&dir_path));
        assert!(app.nodes.len() > before, "child should become visible");

        app.handle_mouse(click(1, dir_idx as u16));
        assert!(!app.expanded.contains(&dir_path));
        assert_eq!(app.nodes.len(), before);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn left_click_respects_scroll_offset() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 1; // first visible row is node index 1

        app.handle_mouse(click(1, 0));
        assert_eq!(app.tree_selected, 1);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn click_below_last_node_is_ignored() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.tree_offset = 0;
        app.tree_selected = 0;

        // Row far past the last node.
        app.handle_mouse(click(1, 18));
        assert_eq!(app.tree_selected, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scroll_wheel_moves_tree_selection() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.tree_area = full_rect();
        app.content_area = Rect {
            x: 100,
            y: 0,
            width: 40,
            height: 20,
        };
        app.tree_selected = 0;

        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.tree_selected, 1);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.tree_selected, 0);
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn scroll_wheel_scrolls_content() {
        let root = temp_tree();
        let mut app = app_for(&root);
        app.open_file(&root.join("long.txt"));
        app.content_area = full_rect();
        app.tree_area = Rect {
            x: 100,
            y: 0,
            width: 40,
            height: 20,
        };

        app.handle_mouse(mouse(MouseEventKind::ScrollDown, 1, 1));
        assert_eq!(app.content_scroll, 3);
        app.handle_mouse(mouse(MouseEventKind::ScrollUp, 1, 1));
        assert_eq!(app.content_scroll, 0);
        fs::remove_dir_all(&root).ok();
    }

    fn open_file_search(app: &mut App) {
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::empty()));
        assert!(app.search.is_some());
        app.search_area = full_rect();
        app.search_offset = 0;
    }

    #[test]
    fn search_single_click_selects_without_opening() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);

        app.handle_mouse(click(1, 1));
        assert_eq!(app.search.as_ref().unwrap().selected, 1);
        assert!(app.search.is_some(), "single click should not open");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_double_click_opens_result() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);

        let target = app.search.as_ref().unwrap().file_results[0].clone();
        app.handle_mouse(click(1, 0));
        app.handle_mouse(click(1, 0)); // second click, same row, within window

        assert!(
            app.search.is_none(),
            "double click should open and close search"
        );
        assert_eq!(app.current_file.as_deref(), Some(target.as_path()));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_click_on_different_row_does_not_open() {
        let root = temp_tree();
        let mut app = app_for(&root);
        open_file_search(&mut app);
        // Need at least two results for this to be meaningful.
        if app.search.as_ref().unwrap().results_len() >= 2 {
            app.handle_mouse(click(1, 0));
            app.handle_mouse(click(1, 1));
            assert!(
                app.search.is_some(),
                "clicks on different rows must not open"
            );
            assert_eq!(app.search.as_ref().unwrap().selected, 1);
        }
        fs::remove_dir_all(&root).ok();
    }

    // ── git mode ─────────────────────────────────────────────────────────────

    /// Repo with:
    ///   committed.txt  – committed "original", working-tree modified to "modified"
    ///   unchanged.txt  – committed "stable", untouched (must stay invisible in git mode)
    ///   new.txt        – untracked
    ///   sub/nested.txt – committed "nested", working-tree modified (gives sub/ a status)
    fn temp_git_with_changes() -> PathBuf {
        use std::process::Command;
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("tv_git_mode_{}_{n}", std::process::id()));
        fs::create_dir_all(dir.join("sub")).unwrap();
        let git = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.email=t@e.x", "-c", "user.name=T"])
                .args(args)
                .status()
                .unwrap();
        };
        git(&["init", "-q"]);
        fs::write(dir.join("committed.txt"), "original\n").unwrap();
        fs::write(dir.join("unchanged.txt"), "stable\n").unwrap();
        fs::write(dir.join("sub").join("nested.txt"), "nested\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-q", "-m", "init"]);
        fs::write(dir.join("committed.txt"), "modified\n").unwrap();
        fs::write(dir.join("sub").join("nested.txt"), "nested modified\n").unwrap();
        fs::write(dir.join("new.txt"), "brand new\n").unwrap();
        dir.canonicalize().unwrap()
    }

    fn ctrl_g() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)
    }

    fn alt_g() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('g'), KeyModifiers::ALT)
    }

    #[test]
    fn git_mode_filters_tree_to_changed_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        assert!(app.git_mode);
        let names: Vec<&str> = app.nodes.iter().map(|n| n.name.as_str()).collect();
        // Changed items must appear.
        assert!(names.contains(&"committed.txt"), "nodes: {names:?}");
        assert!(names.contains(&"new.txt"), "nodes: {names:?}");
        // Unchanged file must be absent.
        assert!(!names.contains(&"unchanged.txt"), "nodes: {names:?}");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_toggle_off_restores_unchanged_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g()); // on
        app.handle_key(ctrl_g()); // off

        assert!(!app.git_mode);
        assert!(!app.is_diff, "should restore file content view");
        assert!(
            app.nodes.iter().any(|n| n.name == "unchanged.txt"),
            "unchanged file must reappear after exiting git mode"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_auto_expands_dirs_with_changes() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);
        assert!(
            !app.expanded.contains(&root.join("sub")),
            "sub/ starts collapsed"
        );

        app.handle_key(ctrl_g());

        assert!(
            app.expanded.contains(&root.join("sub")),
            "git mode must auto-expand dirs containing changes"
        );
        assert!(
            app.nodes.iter().any(|n| n.path.ends_with("nested.txt")),
            "nested changed file must be visible in git mode tree"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_opens_working_tree_diff() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        // Navigate past any leading directory nodes to land on a file.
        // (tree.rs sorts dirs first, so sub/ may be at index 0.)
        let file_idx = app
            .nodes
            .iter()
            .position(|n| !n.is_dir)
            .expect("git mode must have at least one file node");
        for _ in 0..file_idx {
            app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));
        }

        assert!(
            app.is_diff,
            "selecting a file in git mode must show working-tree diff"
        );
        assert!(
            app.content_title
                .as_deref()
                .unwrap_or("")
                .contains("working diff"),
            "title was {:?}",
            app.content_title
        );
        assert!(
            app.content.iter().any(|l| l.starts_with('+')),
            "diff must contain added lines"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_navigation_shows_diff_for_each_file() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        // Move to the next file node.
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::empty()));

        assert!(
            app.is_diff,
            "navigation in git mode must keep showing diffs"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_shows_depth_zero_files() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        app.handle_key(alt_g());

        assert!(app.git_mode_flat);
        assert!(
            app.nodes.iter().all(|n| n.depth == 0 && !n.is_dir),
            "flat mode must only have depth-0 file nodes"
        );
        // Root-level file appears as bare name; nested file as relative path.
        assert!(app.nodes.iter().any(|n| n.name == "committed.txt"));
        assert!(app.nodes.iter().any(|n| n.name.contains("nested.txt")));
        // Unchanged file still absent.
        assert!(!app.nodes.iter().any(|n| n.name.contains("unchanged.txt")));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_toggle_returns_to_tree_view() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());
        app.handle_key(alt_g()); // flat
        app.handle_key(alt_g()); // back to tree

        assert!(app.git_mode);
        assert!(!app.git_mode_flat);
        assert!(
            app.nodes.iter().any(|n| n.is_dir),
            "tree view should include directory nodes"
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_flat_key_is_noop_outside_git_mode() {
        let root = temp_git_with_changes();
        let mut app = app_for(&root);
        let count = app.nodes.len();

        app.handle_key(alt_g());

        assert!(!app.git_mode_flat);
        assert!(!app.git_mode);
        assert_eq!(app.nodes.len(), count, "tree must be unchanged");
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_outside_repo_gives_empty_tree() {
        let root = temp_tree(); // not a git repo
        let mut app = app_for(&root);

        app.handle_key(ctrl_g());

        assert!(app.git_mode);
        assert!(
            app.nodes.is_empty(),
            "no git changes → empty filtered tree; got {} nodes",
            app.nodes.len()
        );
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn git_mode_config_starts_enabled() {
        let root = temp_git_with_changes();
        let cfg = Config {
            git_mode: true,
            ..Config::default()
        };
        let app = App::new(root.to_path_buf(), cfg, None).unwrap();

        assert!(app.git_mode);
        assert!(
            !app.nodes.iter().any(|n| n.name == "unchanged.txt"),
            "unchanged file must be absent when starting in git mode"
        );
        assert!(
            app.nodes.iter().any(|n| n.name == "committed.txt"),
            "changed file must be visible when starting in git mode"
        );
        fs::remove_dir_all(&root).ok();
    }
}
