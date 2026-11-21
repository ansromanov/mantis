use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::app::App;
use crate::code_intel::{Diagnostic, DiagnosticSeverity};
use crate::config::Config;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn temp_dir() -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_lint_{}_{n}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir.canonicalize().unwrap()
}

fn app_for(root: &std::path::Path) -> App {
    App::new(root.to_path_buf(), Config::default(), None, None).unwrap()
}

fn diag(line: usize, severity: DiagnosticSeverity) -> Diagnostic {
    Diagnostic {
        line,
        column: 0,
        end_line: None,
        end_column: None,
        severity,
        message: "m".into(),
        source: "ruff:X".into(),
    }
}

#[test]
fn fresh_lint_result_is_applied_with_strongest_severity_per_line() {
    let root = temp_dir();
    let path = root.join("a.py");
    let mut app = app_for(&root);
    app.latest_diagnostic_requests.insert(path.clone(), 7);
    app.diagnostics_tx
        .send((
            7,
            path.clone(),
            vec![
                diag(3, DiagnosticSeverity::Warning),
                diag(3, DiagnosticSeverity::Error),
                diag(5, DiagnosticSeverity::Info),
            ],
        ))
        .unwrap();
    app.drain_diagnostics();

    assert_eq!(app.file_diagnostics[&path].len(), 3);
    let lines = &app.file_diagnostic_lines[&path];
    assert_eq!(lines[&3], DiagnosticSeverity::Error);
    assert_eq!(lines[&5], DiagnosticSeverity::Info);
    assert!(!app.latest_diagnostic_requests.contains_key(&path));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn stale_lint_result_is_ignored() {
    let root = temp_dir();
    let path = root.join("a.py");
    let mut app = app_for(&root);
    app.latest_diagnostic_requests.insert(path.clone(), 9);
    app.diagnostics_tx
        .send((8, path.clone(), vec![diag(1, DiagnosticSeverity::Error)]))
        .unwrap();
    app.drain_diagnostics();

    assert!(!app.file_diagnostics.contains_key(&path));
    assert_eq!(
        app.latest_diagnostic_requests.get(&path),
        Some(&9),
        "the newer request must stay pending"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn empty_lint_result_leaves_no_diagnostics() {
    let root = temp_dir();
    let path = root.join("a.py");
    let mut app = app_for(&root);
    app.latest_diagnostic_requests.insert(path.clone(), 1);
    app.diagnostics_tx.send((1, path.clone(), vec![])).unwrap();
    app.drain_diagnostics();
    assert!(!app.file_diagnostics.contains_key(&path));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn requesting_for_a_non_python_file_registers_nothing_and_clears_old_data() {
    let root = temp_dir();
    let path = root.join("notes.txt");
    let mut app = app_for(&root);
    app.file_diagnostics
        .insert(path.clone(), vec![diag(0, DiagnosticSeverity::Error)]);
    app.request_file_diagnostics(&path);
    assert!(!app.file_diagnostics.contains_key(&path));
    assert!(!app.latest_diagnostic_requests.contains_key(&path));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn requesting_for_a_python_file_registers_a_pending_request() {
    let root = temp_dir();
    let path = root.join("a.py");
    fs::write(&path, "import os\n").unwrap();
    let mut app = app_for(&root);
    app.request_file_diagnostics(&path);
    assert!(app.latest_diagnostic_requests.contains_key(&path));
    fs::remove_dir_all(&root).ok();
}
