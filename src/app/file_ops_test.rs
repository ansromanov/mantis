use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_file_ops_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

// -- push_recent ------------------------------------------------------------

#[test]
fn push_recent_adds_to_front() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(b.clone());
    assert_eq!(app.recent_ring[0], b);
    assert_eq!(app.recent_ring[1], a);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_deduplicates() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "a\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(a.clone());
    assert_eq!(app.recent_ring.len(), 1);
    assert_eq!(app.recent_ring[0], a);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_moves_existing_to_front() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.push_recent(a.clone());
    app.push_recent(b.clone());
    assert_eq!(app.recent_ring[0], b);
    // Re-pushing a moves it to the front
    app.push_recent(a.clone());
    assert_eq!(app.recent_ring[0], a);
    assert_eq!(app.recent_ring[1], b);
    assert_eq!(app.recent_ring.len(), 2);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn push_recent_caps_at_recent_files_count() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.config.recent_files_count = 3;
    for i in 0..5usize {
        let p = root.join(format!("{i}.txt"));
        fs::write(&p, "x\n").unwrap();
        app.push_recent(p);
    }
    assert_eq!(app.recent_ring.len(), 3);
    fs::remove_dir_all(&root).ok();
}

// -- open_recent_files ------------------------------------------------------

#[test]
fn open_recent_files_empty_ring_does_nothing() {
    let root = temp_dir();
    let mut app = app_for(&root);
    app.open_recent_files();
    assert!(app.recent_files.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_recent_files_excludes_current_file() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "a\n").unwrap();
    let mut app = app_for(&root);
    app.recent_ring = vec![a.clone()];
    app.current_file = Some(a);
    app.open_recent_files();
    // All entries are the current file, so overlay stays closed.
    assert!(app.recent_files.is_none());
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_recent_files_opens_overlay_with_non_current_paths() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "a\n").unwrap();
    fs::write(&b, "b\n").unwrap();
    let mut app = app_for(&root);
    app.recent_ring = vec![a.clone(), b.clone()];
    app.current_file = Some(a);
    app.open_recent_files();
    let state = app.recent_files.as_ref().unwrap();
    assert_eq!(state.paths.len(), 1);
    assert_eq!(state.paths[0], b);
    fs::remove_dir_all(&root).ok();
}

// -- active_line / show_line_blame reset on navigation ----------------------

#[test]
fn open_different_file_resets_active_line_and_blame_popup() {
    let root = temp_dir();
    let a = root.join("a.txt");
    let b = root.join("b.txt");
    fs::write(&a, "line1\nline2\n").unwrap();
    fs::write(&b, "other\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&a);
    app.active_line = 5;
    app.show_line_blame = true;
    app.open_file(&b);
    assert_eq!(app.active_line, 0, "active_line must reset on file open");
    assert!(
        !app.show_line_blame,
        "show_line_blame must close on different file open"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn reopen_same_file_preserves_blame_popup_and_active_line() {
    let root = temp_dir();
    let f = root.join("same.txt");
    fs::write(&f, "line1\nline2\nline3\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&f);
    app.active_line = 1;
    app.show_line_blame = true;
    // Simulate a same-file reload (e.g. watcher tick -> reopen_file).
    app.open_file(&f);
    assert!(
        app.show_line_blame,
        "blame popup stays open when reloading the same file"
    );
    assert_eq!(
        app.active_line, 1,
        "active_line must not reset on same-file reload (blame would show wrong line)"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn failed_reload_does_not_break_same_file_detection() {
    // Sequence: open A, reload fails (file gone), reload succeeds → blame preserved.
    let root = temp_dir();
    let f = root.join("f.txt");
    fs::write(&f, "line1\nline2\nline3\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&f);
    app.active_line = 2;
    app.show_line_blame = true;
    // Simulate a failed reload: file is temporarily absent.
    fs::remove_file(&f).unwrap();
    app.open_file(&f); // load.ok=false → must not corrupt current_file
    fs::write(&f, "line1\nline2\nline3\n").unwrap();
    app.open_file(&f); // successful reload → is_new_file must be false
    assert!(
        app.show_line_blame,
        "blame popup must survive failed-then-successful reload of same file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_keeps_plugin_open_guard_false() {
    // A user-initiated open must leave `plugin_is_opening_file` cleared so the
    // `on_file_open` notification is still emitted to plugins. The guard is set
    // only around plugin-originated opens (see refresh.rs).
    let root = temp_dir();
    let f = root.join("a.txt");
    fs::write(&f, "line1\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&f);
    assert!(
        !app.plugin_is_opening_file,
        "guard must stay false after a normal open_file"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_sets_current_syntax_from_load() {
    let root = temp_dir();
    let f = root.join("main.rs");
    fs::write(&f, "fn main() {}\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&f);
    assert_eq!(
        app.current_syntax.as_deref(),
        Some("Rust"),
        "current_syntax should reflect detected language after file open"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_marks_session_dirty() {
    let root = temp_dir();
    let a = root.join("a.txt");
    fs::write(&a, "line1\nline2\n").unwrap();
    let mut app = app_for(&root);
    app.session_dirty = false;
    app.open_file(&a);
    assert!(
        app.session_dirty,
        "opening a file must mark the session dirty so the new current_file persists"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn open_file_clears_current_syntax_for_unknown_type() {
    let root = temp_dir();
    let rs = root.join("main.rs");
    let unk = root.join("data.zzunknown");
    fs::write(&rs, "fn main() {}\n").unwrap();
    fs::write(&unk, "hello\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&rs);
    assert!(app.current_syntax.is_some(), "should detect Rust");
    app.open_file(&unk);
    assert_eq!(
        app.current_syntax, None,
        "current_syntax should be None for unknown extension"
    );
    fs::remove_dir_all(&root).ok();
}

// -- viewing_revision --------------------------------------------------------

/// Creates a temp git repo with two commits and a working-tree change on
/// `tracked.txt` so file-history operations can be tested:
///   commit 1: tracked.txt = "v1\n"
///   commit 2: tracked.txt = "v2\n"
///   working tree: tracked.txt = "v3\n"
fn temp_git_with_history() -> PathBuf {
    use std::process::Command;
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_viewing_revision_{}_{n}", std::process::id()));
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
    fs::write(dir.join("tracked.txt"), "v1\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "-q", "-m", "first"]);
    fs::write(dir.join("tracked.txt"), "v2\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&["commit", "-q", "-m", "second"]);
    fs::write(dir.join("tracked.txt"), "v3\n").unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn viewing_revision_persists_across_reload_content_in_git_mode() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);

    // Enter git mode and open tracked.txt.
    app.git_mode = true;
    app.open_file(&root.join("tracked.txt"));
    app.show_working_tree_diff(&root.join("tracked.txt"));
    assert!(app.is_diff);
    assert!(
        app.content_title
            .as_deref()
            .unwrap_or("")
            .contains("working diff"),
        "git mode starts with working-tree diff"
    );

    // Fetch commits via file_log and populate history manually.
    let commits = crate::git::file_log(&root, &root.join("tracked.txt"));
    assert!(commits.len() >= 2, "need at least 2 commits for this test");
    let first_short = commits[0].short.clone();

    app.history = Some(crate::search::HistoryState::new(
        root.join("tracked.txt"),
        commits,
    ));
    app.show_selected_revision();

    assert!(
        app.viewing_revision.is_some(),
        "viewing_revision must be set after show_selected_revision"
    );
    assert_eq!(
        app.viewing_revision.as_deref(),
        Some(first_short.as_str()),
        "viewing_revision should match the selected commit short hash"
    );
    let title_before = app.content_title.clone();
    assert!(
        title_before.as_deref().unwrap_or("").contains(&first_short),
        "title before: {:?}",
        title_before
    );

    // reload_content must NOT clobber the revision diff.
    app.reload_content();

    assert!(
        app.viewing_revision.is_some(),
        "viewing_revision must survive reload_content"
    );
    assert_eq!(
        app.content_title, title_before,
        "content title must not change after reload_content"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn viewing_revision_persists_across_reload_content_in_normal_mode() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);

    // Open file in normal mode.
    app.open_file(&root.join("tracked.txt"));
    assert!(!app.is_diff, "not a diff in normal mode");

    // Fetch commits and populate history.
    let commits = crate::git::file_log(&root, &root.join("tracked.txt"));
    assert!(commits.len() >= 2, "need at least 2 commits");
    let first_short = commits[0].short.clone();

    app.history = Some(crate::search::HistoryState::new(
        root.join("tracked.txt"),
        commits,
    ));
    app.show_selected_revision();

    assert!(app.viewing_revision.is_some());
    assert_eq!(
        app.viewing_revision.as_deref(),
        Some(first_short.as_str()),
        "viewing_revision should match the selected commit short hash"
    );
    let title_before = app.content_title.clone();

    app.reload_content();

    assert!(
        app.viewing_revision.is_some(),
        "viewing_revision must survive reload_content in normal mode"
    );
    assert_eq!(
        app.content_title, title_before,
        "content title must not change"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn viewing_revision_cleared_by_open_file() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);
    let other = root.join("other.txt");
    fs::write(&other, "hello\n").unwrap();

    // Set state as if a revision is being viewed.
    app.viewing_revision = Some("abc1234".to_string());
    app.open_file(&other);

    assert!(
        app.viewing_revision.is_none(),
        "open_file must clear viewing_revision"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn viewing_revision_cleared_by_reload_key() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);
    app.open_file(&root.join("tracked.txt"));

    // Simulate viewing a revision.
    app.viewing_revision = Some("abc1234".to_string());

    // Press reload key (r).
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('r'),
        crossterm::event::KeyModifiers::empty(),
    ));

    assert!(
        app.viewing_revision.is_none(),
        "reload key must clear viewing_revision"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn viewing_revision_cleared_by_esc_in_git_mode() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);

    // Enter git mode and show working-tree diff.
    app.git_mode = true;
    let file = root.join("tracked.txt");
    app.open_file(&file);
    app.show_working_tree_diff(&file);

    // Simulate viewing a revision.
    app.viewing_revision = Some("abc1234".to_string());
    app.current_file = Some(file.clone());

    // Press Esc.
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::empty(),
    ));

    assert!(
        app.viewing_revision.is_none(),
        "Esc must clear viewing_revision in git mode"
    );
    assert!(
        app.content_title
            .as_deref()
            .unwrap_or("")
            .contains("working diff"),
        "Esc should restore the working-tree diff in git mode"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn viewing_revision_cleared_by_esc_in_normal_mode() {
    let root = temp_git_with_history();
    let mut app = app_for(&root);
    let file = root.join("tracked.txt");
    app.open_file(&file);

    // Simulate viewing a revision.
    app.viewing_revision = Some("abc1234".to_string());

    // Press Esc.
    app.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Esc,
        crossterm::event::KeyModifiers::empty(),
    ));

    assert!(
        app.viewing_revision.is_none(),
        "Esc must clear viewing_revision in normal mode"
    );
    assert!(
        !app.is_diff,
        "Esc should restore the file content view in normal mode"
    );
    fs::remove_dir_all(&root).ok();
}

// -- highlight cache invalidation -------------------------------------------

#[test]
fn reload_content_clears_highlight_cache() {
    let root = temp_dir();
    let path = root.join("file.txt");
    fs::write(&path, "hello\n").unwrap();
    let mut app = app_for(&root);
    app.open_file(&path);
    *app.content_highlight_cache.borrow_mut() = Some((
        crate::app::HighlightCacheKey {
            path: path.clone(),
            scroll: 0,
            visible_end: 1,
            theme: app.theme.syntax.clone(),
            word_wrap: app.word_wrap,
        },
        vec![vec![(
            ratatui::style::Style::default(),
            "hello".to_string(),
        )]],
    ));
    assert!(
        app.content_highlight_cache.borrow().is_some(),
        "precondition"
    );
    app.reload_content();
    assert!(
        app.content_highlight_cache.borrow().is_none(),
        "reload_content must clear the highlight cache"
    );
    fs::remove_dir_all(&root).ok();
}
