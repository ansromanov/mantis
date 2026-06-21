use super::*;

use std::io::Cursor;

#[test]
fn shutdown_terminates_loop() {
    let input = Cursor::new(b"{\"event\":\"shutdown\"}\n");
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    run_loop(input, &mut output, "/tmp/tv-git-diff-test", &mut tmp_files);
    // If we reach here, the loop terminated without panic.
}

#[test]
fn on_file_open_with_tmp_prefix_is_skipped() {
    let input = Cursor::new(
        b"{\"event\":\"on_file_open\",\"path\":\"/tmp/tv-git-diff-test-abc\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    run_loop(input, &mut output, "/tmp/tv-git-diff-test", &mut tmp_files);
    // No open_file action should be emitted for the tmp-prefixed path.
    assert!(output.is_empty());
}

#[test]
fn on_file_open_nonexistent_file_produces_no_output() {
    let input = Cursor::new(
        b"{\"event\":\"on_file_open\",\"path\":\"/nonexistent/path/file.rs\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    run_loop(input, &mut output, "/tmp/tv-git-diff-test", &mut tmp_files);
    assert!(output.is_empty());
}

#[test]
fn unknown_events_are_ignored() {
    let input = Cursor::new(
        b"{\"event\":\"init\"}\n\
          {\"event\":\"on_keypress\",\"key\":\"H\"}\n\
          {\"event\":\"shutdown\"}\n",
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    run_loop(input, &mut output, "/tmp/tv-git-diff-test", &mut tmp_files);
    assert!(output.is_empty());
}
