use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::config::Config;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_tree() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_mod_test_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

#[test]
fn enter_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Enter));
    assert!(!app.show_about, "Enter must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_about, "Esc must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn q_closes_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Char('q')));
    assert!(!app.show_about, "q must close the About dialog");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn o_does_not_close_about() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_about = true;
    app.handle_key(key(KeyCode::Char('o')));
    assert!(
        app.show_about,
        "o must NOT close the About dialog (it opens the release URL)"
    );
    fs::remove_dir_all(&root).ok();
}

// -- protocol 3: on_keypress key consumption -----------------------------------

/// Replaces `app.plugin_manager` with a fresh one running a single spawned
/// stub plugin subscribed to `on_keypress`. When `reply` is `true` the stub
/// replies `key_handled: true` to every `on_keypress` it receives; otherwise
/// it reads and discards input, never replying.
#[cfg(unix)]
fn spawn_keypress_responder(app: &mut App, dir: &std::path::Path, reply: bool) {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;

    let script = dir.join("kp.sh");
    let mut f = fs::File::create(&script).unwrap();
    if reply {
        write!(
            f,
            "#!/bin/sh\nwhile read -r line; do\n  case \"$line\" in\n    *'\"event\":\"on_keypress\"'*)\n      echo '{{\"event\":\"action\",\"action\":\"key_handled\",\"params\":{{\"handled\":true}}}}'\n      ;;\n  esac\ndone\n"
        )
        .unwrap();
    } else {
        write!(f, "#!/bin/sh\ncat > /dev/null\n").unwrap();
    }
    drop(f);
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let entry = crate::plugin::PluginEntry {
        path: script,
        enabled: false,
        events: vec!["on_keypress".to_string()],
        ..Default::default()
    };
    app.plugin_manager = crate::plugin::PluginManager::new(vec![("kp".to_string(), entry)]);
    app.plugin_manager
        .activate_one("kp", None)
        .expect("spawn kp.sh");
}

/// Drives `app.tick()` until `pending_keypress` resolves (or panics past a
/// generous deadline), mirroring the ~16ms poll loop in `main.rs` without a
/// real 16ms sleep per iteration.
fn drain_until_keypress_resolved(app: &mut App) {
    // Deadline comfortably exceeds KEY_CONSUME_TIMEOUT (2s under cfg(test))
    // so a slow subprocess round trip under parallel test-suite load doesn't
    // race the timeout path.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.pending_keypress.is_some() {
        app.tick();
        assert!(
            std::time::Instant::now() < deadline,
            "pending keypress was never resolved"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn on_keypress_no_subscribers_falls_through_immediately() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    let before = app.tree_selected;

    app.handle_key(key(KeyCode::Down));

    assert!(
        app.pending_keypress.is_none(),
        "no on_keypress subscribers: nothing should be deferred"
    );
    assert_ne!(
        app.tree_selected, before,
        "with no plugin subscribers, behavior must be unchanged from before protocol 3"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(unix)]
fn on_keypress_subscriber_replies_handled_swallows_key() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    spawn_keypress_responder(&mut app, &root, true);
    let before = app.tree_selected;

    app.handle_key(key(KeyCode::Down));
    assert!(
        app.pending_keypress.is_some(),
        "key must be deferred while a subscriber might claim it"
    );

    drain_until_keypress_resolved(&mut app);

    assert_eq!(
        app.tree_selected, before,
        "a key claimed via key_handled must not run normal-mode handling"
    );

    app.plugin_manager.deactivate_all();
    fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(unix)]
fn on_keypress_subscriber_never_replies_falls_through_after_deadline() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    spawn_keypress_responder(&mut app, &root, false);
    let before = app.tree_selected;

    app.handle_key(key(KeyCode::Down));
    assert!(app.pending_keypress.is_some());

    drain_until_keypress_resolved(&mut app);

    assert_ne!(
        app.tree_selected, before,
        "an unclaimed key must fall through to normal-mode handling after the deadline"
    );

    app.plugin_manager.deactivate_all();
    fs::remove_dir_all(&root).ok();
}

#[test]
#[cfg(unix)]
fn on_keypress_new_key_preempts_stale_pending_keypress() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    spawn_keypress_responder(&mut app, &root, false);

    app.handle_key(key(KeyCode::Down));
    assert!(app.pending_keypress.is_some());
    let before = app.tree_selected;

    // A second key arrives before the first key's deadline: the stale
    // pending key must be resolved (fallthrough) immediately, not dropped.
    app.handle_key(key(KeyCode::Down));
    assert_ne!(
        app.tree_selected, before,
        "the preempted key must fall through immediately rather than being lost"
    );

    app.plugin_manager.deactivate_all();
    fs::remove_dir_all(&root).ok();
}

// -- welcome overlay key handling --------------------------------------------

#[test]
fn esc_dismisses_welcome() {
    let root = temp_tree();
    let mut app = app_for(&root);
    app.show_welcome = true;
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_welcome, "Esc must dismiss the welcome overlay");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn non_esc_keys_are_blocked_by_welcome() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);

    // Enable welcome, then press Down — it should NOT navigate tree.
    app.show_welcome = true;
    let before = app.tree_selected;
    app.handle_key(key(KeyCode::Down));
    assert_eq!(
        app.tree_selected, before,
        "non-Esc keys must be blocked (selected remained {before})"
    );

    // Welcome is still shown after non-Esc key.
    assert!(app.show_welcome, "welcome must survive non-Esc keys");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn esc_dismisses_welcome_and_normal_keys_still_work() {
    let root = temp_tree();
    fs::write(root.join("a.txt"), "").unwrap();
    fs::write(root.join("b.txt"), "").unwrap();
    let mut app = app_for(&root);
    app.show_welcome = true;

    // Dismiss the welcome overlay.
    app.handle_key(key(KeyCode::Esc));
    assert!(!app.show_welcome, "welcome dismissed");

    // Now pressing Down should move the tree selection.
    let before = app.tree_selected;
    app.handle_key(key(KeyCode::Down));
    assert!(
        app.tree_selected > before || app.tree_selected >= app.nodes.len() - 1,
        "after dismiss, Down must navigate tree (selected changed from {before} to {})",
        app.tree_selected
    );
    fs::remove_dir_all(&root).ok();
}
