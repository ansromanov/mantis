use super::*;

use std::io::Cursor;

fn run(input: &[u8]) -> (Vec<u8>, Option<String>, Option<String>) {
    let cursor = Cursor::new(input.to_vec());
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file = None;
    let mut last_sel_file = None;
    run_loop(
        cursor,
        &mut output,
        "/tmp/tv-git-diff-test",
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
        &mut last_sel_file,
    );
    (output, last_file, last_sel_file)
}

#[test]
fn shutdown_terminates_loop() {
    let (output, _, _) = run(b"{\"event\":\"shutdown\"}\n");
    assert!(output.is_empty());
}

#[test]
fn on_file_open_updates_last_file() {
    let (_, last_file, _) = run(b"{\"event\":\"on_file_open\",\"path\":\"/some/file.rs\"}\n\
          {\"event\":\"shutdown\"}\n");
    assert_eq!(last_file.as_deref(), Some("/some/file.rs"));
}

#[test]
fn on_selection_change_deduplicates() {
    let cursor = Cursor::new(
        b"{\"event\":\"on_selection_change\",\"path\":\"/a.rs\"}\n\
          {\"event\":\"on_selection_change\",\"path\":\"/a.rs\"}\n\
          {\"event\":\"shutdown\"}\n"
            .to_vec(),
    );
    let mut output = Vec::new();
    let mut tmp_files = Vec::new();
    let mut last_file = None;
    let mut last_sel_file = None;
    run_loop(
        cursor,
        &mut output,
        "/tmp/tv-git-diff-test",
        "/tmp/tv-git-log-test",
        &mut tmp_files,
        &mut last_file,
        &mut last_sel_file,
    );
    // The second identical selection should be deduplicated; last_sel_file is set.
    assert_eq!(last_sel_file.as_deref(), Some("/a.rs"));
}

#[test]
fn keypress_h_without_last_file_produces_no_output() {
    let (output, _, _) =
        run(b"{\"event\":\"on_keypress\",\"key\":\"H\"}\n{\"event\":\"shutdown\"}\n");
    assert!(output.is_empty());
}

#[test]
fn keypress_b_without_last_file_produces_no_output() {
    let (output, _, _) =
        run(b"{\"event\":\"on_keypress\",\"key\":\"b\"}\n{\"event\":\"shutdown\"}\n");
    assert!(output.is_empty());
}

#[test]
fn send_repo_info_on_non_git_path_is_silent() {
    let mut output = Vec::new();
    send_repo_info("/nonexistent/path/file.rs", &mut output);
    assert!(output.is_empty());
}

#[test]
fn send_file_statuses_on_non_git_path_is_silent() {
    let mut output = Vec::new();
    send_file_statuses("/nonexistent/path/file.rs", &mut output);
    assert!(output.is_empty());
}

#[test]
fn send_blame_data_on_nonexistent_file_is_silent() {
    let mut output = Vec::new();
    send_blame_data("/nonexistent/path/file.rs", &mut output);
    assert!(output.is_empty());
}
