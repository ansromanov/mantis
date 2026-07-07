use super::*;

use std::fs;

fn read_lines(dir: &std::path::Path) -> Vec<serde_json::Value> {
    let raw = fs::read_to_string(dir.join("events.jsonl")).unwrap();
    raw.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn disabled_handle_is_noop() {
    let t = Telemetry::disabled();
    assert!(!t.is_enabled());
    t.record(TelemetryEvent::ActionInvoked {
        action: "quit",
        source: ActionSource::Palette,
    });
    // Dropping must not panic or create anything; nothing observable to
    // assert beyond the absence of a writer thread (inner is None).
    assert!(t.inner.is_none());
    drop(t);
}

#[test]
fn new_disabled_creates_no_state_files() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", tmp.path());
    let t = Telemetry::new(false);
    assert!(!t.is_enabled());
    drop(t);
    assert!(!tmp.path().join("telemetry").exists());
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn new_enabled_writes_under_state_dir_telemetry() {
    let _guard = crate::session::STATE_DIR_ENV_LOCK.lock().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    std::env::set_var("MANTIS_STATE_DIR", tmp.path());
    let t = Telemetry::new(true);
    assert!(t.is_enabled());
    drop(t); // joins the writer, flushing buffered events
    assert!(tmp.path().join("telemetry").join("events.jsonl").exists());
    std::env::remove_var("MANTIS_STATE_DIR");
}

#[test]
fn session_lifecycle_and_action_events_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let t = Telemetry::with_dir(tmp.path().to_path_buf());
    t.record(TelemetryEvent::ActionInvoked {
        action: "toggle_hidden",
        source: ActionSource::Palette,
    });
    drop(t);

    let lines = read_lines(tmp.path());
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["event"], "session_start");
    assert_eq!(lines[0]["app_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(lines[0]["os"], std::env::consts::OS);
    assert_eq!(lines[1]["event"], "action_invoked");
    assert_eq!(lines[1]["action"], "toggle_hidden");
    assert_eq!(lines[1]["source"], "palette");
    assert_eq!(lines[2]["event"], "session_end");
    assert_eq!(lines[2]["events_dropped"], 0);
    for line in &lines {
        assert!(line["ts_ms"].is_u64(), "every event is stamped: {line}");
    }
}

#[test]
fn events_contain_only_whitelisted_keys() {
    let tmp = tempfile::tempdir().unwrap();
    let t = Telemetry::with_dir(tmp.path().to_path_buf());
    t.record(TelemetryEvent::ActionInvoked {
        action: "help",
        source: ActionSource::Palette,
    });
    drop(t);

    let allowed = [
        "event",
        "ts_ms",
        "app_version",
        "os",
        "arch",
        "terminal",
        "duration_s",
        "events_dropped",
        "action",
        "source",
    ];
    for line in read_lines(tmp.path()) {
        for key in line.as_object().unwrap().keys() {
            assert!(allowed.contains(&key.as_str()), "unexpected key {key}");
        }
    }
}
