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
        .filter(|n| n.starts_with("events-") && n.ends_with(".jsonl"))
        .collect();
    names.sort();
    names
}

#[test]
fn append_writes_one_stamped_json_line() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sink = JsonlSink::new(tmp.path().to_path_buf());
    sink.append(&event(), Duration::from_millis(42));

    let raw = fs::read_to_string(tmp.path().join("events.jsonl")).unwrap();
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
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 80, 4);
    sink.append(&event(), Duration::ZERO);
    sink.append(&event(), Duration::ZERO);

    assert_eq!(rotated_files(tmp.path()).len(), 1);
    let active = fs::read_to_string(tmp.path().join("events.jsonl")).unwrap();
    assert_eq!(active.lines().count(), 1, "active restarted after rotation");
}

#[test]
fn prune_keeps_at_most_max_rotated_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 80, 2);
    for _ in 0..6 {
        sink.append(&event(), Duration::ZERO);
    }
    assert!(rotated_files(tmp.path()).len() <= 2);
    assert!(tmp.path().join("events.jsonl").exists());
}

#[test]
fn existing_active_file_length_is_respected_on_reopen() {
    let tmp = tempfile::tempdir().unwrap();
    {
        let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 80, 4);
        sink.append(&event(), Duration::ZERO);
    }
    // A new sink over the same dir sees the existing length and rotates on
    // the next over-cap append instead of growing the file unbounded.
    let mut sink = JsonlSink::with_limits(tmp.path().to_path_buf(), 80, 4);
    sink.append(&event(), Duration::ZERO);
    assert_eq!(rotated_files(tmp.path()).len(), 1);
}
