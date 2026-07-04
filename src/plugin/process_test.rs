use crate::plugin::process::{drain_rest_of_line, read_capped_line, Plugin, MAX_LINE_LEN};
use crate::plugin::types::ToPlugin;

/// Small stand-in for `MAX_LINE_LEN` so cap-boundary tests don't allocate
/// multi-megabyte buffers just to exercise the truncation path.
const TEST_CAP: usize = 16;

#[test]
fn plugin_new_sets_name() {
    let p = Plugin::new("test-p".into(), vec![]);
    assert_eq!(p.name, "test-p");
}

#[test]
fn plugin_new_stores_subscribed_events() {
    let p = Plugin::new("evt-p".into(), vec!["on_file_open".into()]);
    assert!(p.subscribes_to("on_file_open"));
    assert!(!p.subscribes_to("on_keypress"));
}

#[test]
fn plugin_new_empty_events_subscribes_to_all() {
    let p = Plugin::new("all-p".into(), vec![]);
    assert!(p.subscribes_to("on_file_open"));
    assert!(p.subscribes_to("on_keypress"));
    assert!(p.subscribes_to("on_quit"));
}

#[test]
fn drain_actions_returns_empty_when_no_reader() {
    let mut p = Plugin::new("no-reader".into(), vec![]);
    let (actions, is_dead) = p.drain_actions();
    assert!(actions.is_empty());
    assert!(is_dead, "no reader channel means the plugin is dead");
}

#[test]
fn send_is_noop_when_no_writer() {
    let mut p = Plugin::new("no-writer".into(), vec![]);
    p.send(&ToPlugin {
        event: "test".into(),
        path: None,
        key: None,
        theme: None,
        protocol_version: None,
    });
}

#[test]
fn read_capped_line_reads_full_line_within_cap() {
    let data = b"hello world\nnext\n";
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    assert!(read_capped_line(&mut reader, &mut buf, MAX_LINE_LEN));
    assert_eq!(buf, b"hello world\n");
}

#[test]
fn read_capped_line_returns_true_for_final_unterminated_line() {
    let data = b"no newline here";
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    // EOF reached with bytes buffered: the final line is still processed.
    assert!(read_capped_line(&mut reader, &mut buf, MAX_LINE_LEN));
    assert_eq!(buf, b"no newline here");
}

#[test]
fn read_capped_line_reads_document_sized_line() {
    // Regression: the markdown plugin emits a fully rendered document as one
    // `set_content` line (~70 KB for a large file with wide tables). The cap
    // must be large enough to read it in full; a 64 KB cap silently dropped
    // such messages, so the file rendered as raw source.
    let big = 100_000;
    assert!(MAX_LINE_LEN >= big, "cap too small for a rendered document");
    let mut data = vec![b'a'; big];
    data.push(b'\n');
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    assert!(read_capped_line(&mut reader, &mut buf, MAX_LINE_LEN));
    assert_eq!(buf.len(), big + 1);
}

#[test]
fn read_capped_line_truncates_overlength_line() {
    // A line longer than the cap with the newline well past it must not
    // exceed the cap, even when the newline is visible in the buffer.
    let mut data = vec![b'a'; TEST_CAP + 5];
    data.push(b'\n');
    data.extend_from_slice(b"after\n");
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    // Truncated: no newline accepted within the cap.
    assert!(!read_capped_line(&mut reader, &mut buf, TEST_CAP));
    assert_eq!(buf.len(), TEST_CAP);

    // Caller drains the rest of the overlength line, then the next line reads
    // cleanly.
    drain_rest_of_line(&mut reader);
    buf.clear();
    assert!(read_capped_line(&mut reader, &mut buf, TEST_CAP));
    assert_eq!(buf, b"after\n");
}

#[test]
fn read_capped_line_does_not_overshoot_when_newline_past_cap() {
    // Regression: a chunk that contains a newline whose position exceeds the
    // remaining cap budget must not be copied past the cap. Pre-fill buf
    // close to the cap so remaining is tiny, then feed a short line whose
    // newline sits beyond `remaining`.
    let mut buf = vec![b'x'; TEST_CAP - 3];
    let data = b"abcde\nrest\n";
    let mut reader = std::io::BufReader::new(&data[..]);
    // newline at index 5 > remaining (3): truncated, buf must equal exactly cap.
    assert!(!read_capped_line(&mut reader, &mut buf, TEST_CAP));
    assert_eq!(buf.len(), TEST_CAP);
}

/// Shared crate-wide lock serialising every test that sets `MANTIS_STATE_DIR`
/// (a process-global env var) — see [`crate::session::STATE_DIR_ENV_LOCK`].
#[cfg(unix)]
use crate::session::STATE_DIR_ENV_LOCK as STATE_DIR_LOCK;

#[test]
#[cfg(unix)]
fn spawn_captures_stderr_to_log_and_last_line() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_stderr_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\necho 'boom: something broke' >&2\nexit 1\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut p = Plugin::new("crash-plugin".into(), vec![]);
    p.spawn(&script).expect("spawn crash.sh");

    // Wait for the reader thread to observe the process exit (stdout closes).
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, is_dead) = p.drain_actions();
        if is_dead {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never reported dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    // The stderr-drain thread runs concurrently; give it a moment to catch up.
    let deadline = Instant::now() + Duration::from_secs(2);
    while p.last_stderr_line().is_none() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }

    assert_eq!(
        p.last_stderr_line().as_deref(),
        Some("boom: something broke"),
        "last stderr line must be captured for the death message"
    );
    let log_path = p.log_path().expect("log path must be recorded");
    let contents = std::fs::read_to_string(&log_path).expect("log file must exist");
    assert!(
        contents.contains("boom: something broke"),
        "log file must contain the plugin's stderr output, got: {contents:?}"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(unix)]
fn spawn_sanitizes_path_traversal_in_plugin_name_for_log_file() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_stderr_traversal_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(f, "#!/bin/sh\necho boom >&2\nexit 1\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    // A plugin name is a `[plugins]` config key, not validated for filesystem
    // safety; it must not be able to escape `plugin-logs/` via `..` or an
    // absolute path when used to build the log filename.
    let mut p = Plugin::new("../../etc/evil".into(), vec![]);
    p.spawn(&script).expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, is_dead) = p.drain_actions();
        if is_dead {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never reported dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let log_path = p.log_path().expect("log path must be recorded");
    let logs_dir = state_dir.join("plugin-logs");
    assert_eq!(
        log_path.parent(),
        Some(logs_dir.as_path()),
        "log path must stay inside plugin-logs/, got: {log_path:?}"
    );
    assert!(
        log_path.exists(),
        "sanitized log file must have been created at {log_path:?}"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(unix)]
fn spawn_preserves_leading_indentation_in_stderr_log() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_stderr_indent_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    // A stack-trace-like line indented with tabs: indentation must survive,
    // since that's exactly the content this diagnostics feature exists to
    // preserve.
    write!(f, "#!/bin/sh\nprintf '\\tat foo.rs:42\\n' >&2\nexit 1\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut p = Plugin::new("indent-crash-plugin".into(), vec![]);
    p.spawn(&script).expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, is_dead) = p.drain_actions();
        if is_dead {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never reported dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let line = p
        .last_stderr_line()
        .expect("last stderr line must be captured");
    assert_eq!(
        line, "\tat foo.rs:42",
        "leading indentation must be preserved, not trimmed away"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(unix)]
fn spawn_strips_ansi_escapes_from_last_stderr_line() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_stderr_ansi_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    // Emits a stderr line containing a CSI escape sequence (clear screen) and
    // a stray carriage return, mimicking a malicious or buggy plugin trying
    // to smuggle terminal control sequences into the diagnostics UI.
    write!(f, "printf 'boom\\x1b[2Jtail\\r\\n' >&2\nexit 1\n").unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut p = Plugin::new("ansi-crash-plugin".into(), vec![]);
    p.spawn(&script).expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, is_dead) = p.drain_actions();
        if is_dead {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never reported dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let line = p
        .last_stderr_line()
        .expect("last stderr line must be captured");
    assert_eq!(
        line, "boom[2Jtail",
        "control characters (ESC, CR) must be stripped before display"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
#[cfg(unix)]
fn spawn_truncates_oversized_last_stderr_line() {
    use std::io::Write as _;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let _lock = STATE_DIR_LOCK.lock().unwrap();

    let dir = std::env::temp_dir().join(format!("tv_stderr_trunc_test_{}", std::process::id()));
    let state_dir = dir.join("state");
    std::fs::create_dir_all(&state_dir).unwrap();
    std::env::set_var("MANTIS_STATE_DIR", &state_dir);

    let script = dir.join("crash.sh");
    let mut f = std::fs::File::create(&script).unwrap();
    write!(
        f,
        "#!/bin/sh\nyes x | head -c 5000 | tr -d '\\n' >&2\necho >&2\nexit 1\n"
    )
    .unwrap();
    drop(f);
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut p = Plugin::new("longline-crash-plugin".into(), vec![]);
    p.spawn(&script).expect("spawn crash.sh");

    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let (_, is_dead) = p.drain_actions();
        if is_dead {
            break;
        }
        assert!(Instant::now() < deadline, "plugin never reported dead");
        std::thread::sleep(Duration::from_millis(25));
    }

    let line = p
        .last_stderr_line()
        .expect("last stderr line must be captured");
    assert!(
        line.chars().count() <= 301,
        "a runaway stderr line must be truncated for display, got {} chars",
        line.chars().count()
    );
    assert!(
        line.ends_with('…'),
        "truncated line must be marked with an ellipsis, got: {line:?}"
    );

    std::env::remove_var("MANTIS_STATE_DIR");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn read_capped_line_caps_when_newline_exactly_at_boundary() {
    // Newline sits at index TEST_CAP (the byte after the cap window): the
    // content fills the cap exactly and is treated as overlength.
    let mut data = vec![b'b'; TEST_CAP];
    data.push(b'\n');
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    assert!(!read_capped_line(&mut reader, &mut buf, TEST_CAP));
    assert_eq!(buf.len(), TEST_CAP);
}
