use super::*;

use std::io::Cursor;

#[test]
fn shutdown_terminates_loop() {
    let input = Cursor::new(b"{\"event\":\"shutdown\"}\n");
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file = None;
    run_loop(
        input,
        &mut output,
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
    );
    // If we reach here, the loop terminated without panic.
}

#[test]
fn on_file_open_with_tmp_prefix_does_not_update_last_file() {
    let input = Cursor::new(
        b"{\"event\":\"on_file_open\",\"path\":\"/tmp/tv-git-log-123-abc\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file: Option<String> = None;
    run_loop(
        input,
        &mut output,
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
    );
    assert!(last_file.is_none());
}

#[test]
fn on_file_open_updates_last_file() {
    let input = Cursor::new(
        b"{\"event\":\"on_file_open\",\"path\":\"/some/real/file.rs\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file: Option<String> = None;
    run_loop(
        input,
        &mut output,
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
    );
    assert_eq!(last_file.as_deref(), Some("/some/real/file.rs"));
}

#[test]
fn keypress_h_without_last_file_produces_no_output() {
    let input = Cursor::new(
        b"{\"event\":\"on_keypress\",\"key\":\"H\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file: Option<String> = None;
    run_loop(
        input,
        &mut output,
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
    );
    assert!(output.is_empty());
}
