use crate::plugin::process::{drain_rest_of_line, read_capped_line, Plugin, MAX_LINE_LEN};
use crate::plugin::types::ToPlugin;

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
    assert!(read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf, b"hello world\n");
}

#[test]
fn read_capped_line_returns_true_for_final_unterminated_line() {
    let data = b"no newline here";
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    // EOF reached with bytes buffered: the final line is still processed.
    assert!(read_capped_line(&mut reader, &mut buf));
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
    assert!(read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf.len(), big + 1);
}

#[test]
fn read_capped_line_truncates_overlength_line() {
    // A line longer than MAX_LINE_LEN with the newline well past the cap must
    // not exceed the cap, even when the newline is visible in the buffer.
    let mut data = vec![b'a'; MAX_LINE_LEN + 5000];
    data.push(b'\n');
    data.extend_from_slice(b"after\n");
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    // Truncated: no newline accepted within the cap.
    assert!(!read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf.len(), MAX_LINE_LEN);

    // Caller drains the rest of the overlength line, then the next line reads
    // cleanly.
    drain_rest_of_line(&mut reader);
    buf.clear();
    assert!(read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf, b"after\n");
}

#[test]
fn read_capped_line_does_not_overshoot_when_newline_past_cap() {
    // Regression: a chunk that contains a newline whose position exceeds the
    // remaining cap budget must not be copied past MAX_LINE_LEN. Pre-fill buf
    // close to the cap so remaining is tiny, then feed a short line whose
    // newline sits beyond `remaining`.
    let mut buf = vec![b'x'; MAX_LINE_LEN - 3];
    let data = b"abcde\nrest\n";
    let mut reader = std::io::BufReader::new(&data[..]);
    // newline at index 5 > remaining (3): truncated, buf must equal exactly cap.
    assert!(!read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf.len(), MAX_LINE_LEN);
}

#[test]
fn read_capped_line_caps_when_newline_exactly_at_boundary() {
    // Newline sits at index MAX_LINE_LEN (the byte after the cap window): the
    // content fills the cap exactly and is treated as overlength.
    let mut data = vec![b'b'; MAX_LINE_LEN];
    data.push(b'\n');
    let mut reader = std::io::BufReader::new(&data[..]);
    let mut buf = Vec::new();
    assert!(!read_capped_line(&mut reader, &mut buf));
    assert_eq!(buf.len(), MAX_LINE_LEN);
}
