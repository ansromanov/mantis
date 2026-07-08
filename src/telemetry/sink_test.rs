use super::*;

use std::fs;
use std::path::Path;

fn event() -> TelemetryEvent {
    TelemetryEvent::ActionInvoked {
        action: "quit",
        source: crate::telemetry::ActionSource::Palette,
    }
}

fn rotated_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            // Rotated files have a `-<n>` before `.jsonl` (e.g. `events-42-1.jsonl`).
            // The active file is `events-42.jsonl` (no `-<n>` before `.jsonl`).
            if let Some(rest) = n
                .strip_prefix("events-")
                .and_then(|s| s.strip_suffix(".jsonl"))
            {
                rest.contains('-')
            } else {
                false
            }
        })
        .collect();
    names.sort();
    names
}

#[test]
fn append_writes_one_stamped_json_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sink = JsonlSink::new(tmp.path().to_path_buf(), 42);
    sink.append(&event(), Duration::from_millis(42));

    let raw = fs::read_to_string(tmp.path().join("events-42.jsonl")).unwrap();
    let lines: Vec<&str> = raw.lines().collect();
    assert_eq!(lines.len(), 1);
    let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(v["event"], "action_invoked");
    assert_eq!(v["ts_ms"], 42);
}

#[test]
fn append_over_cap_rotates_active_file() {
    let tmp = tempfile::tempdir().unwrap();
    // Cap below two lines so the second append must rotate first.
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 42, 80, 4);
    sink.append(&event(), Duration::ZERO);
    sink.append(&event(), Duration::ZERO);

    assert_eq!(rotated_files(tmp.path()).len(), 1);
    let active = fs::read_to_string(tmp.path().join("events-42.jsonl")).unwrap();
    assert_eq!(active.lines().count(), 1, "active restarted after rotation");
}

#[test]
fn prune_keeps_at_most_max_rotated_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 42, 80, 2);
    for _ in 0..6 {
        sink.append(&event(), Duration::ZERO);
    }
    assert!(rotated_files(tmp.path()).len() <= 2);
    assert!(tmp.path().join("events-42.jsonl").exists());
}

#[test]
fn collision_on_active_file_uses_incrementing_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    // Pre-create a file with the same epoch name.
    fs::write(tmp.path().join("events-42.jsonl"), "").unwrap();
    let sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 42, 1024, 4);
    assert_eq!(sink.active_name, "events-42-1.jsonl");
}

#[test]
fn collision_on_active_file_rotate_preserves_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    // Pre-create a file with the same epoch name.
    fs::write(tmp.path().join("events-42.jsonl"), "prior session data\n").unwrap();

    // Cap below two lines so the second append must rotate first.
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 42, 80, 4);
    assert_eq!(sink.active_name, "events-42-1.jsonl");

    sink.append(&event(), Duration::ZERO);
    sink.append(&event(), Duration::ZERO);

    // After rotation, active file should still use the suffix base name.
    assert_eq!(sink.active_name, "events-42-1.jsonl");
    assert!(tmp.path().join("events-42-1.jsonl").exists());
    assert!(tmp.path().join("events-42-2.jsonl").exists());

    // The prior session file events-42.jsonl must be untouched.
    let prior_content = fs::read_to_string(tmp.path().join("events-42.jsonl")).unwrap();
    assert_eq!(prior_content, "prior session data\n");
}
