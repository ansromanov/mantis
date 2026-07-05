use std::collections::VecDeque;

use super::*;

// ---------------------------------------------------------------------------
// Unhandled bytes: Ok(None) not Err
// ---------------------------------------------------------------------------

#[test]
fn high_byte_skipped_by_next_raw_event() {
    // parse_event returns Err for 0x80; next_raw_event must convert that to
    // Ok(None) (skip the byte) rather than propagating the error.
    let mut src = RawEventSource::new();
    src.buf.push(0x80);
    src.buf.push(b'q'); // a valid event after the bad byte
    let result = src.next_raw_event();
    assert!(
        result.is_ok(),
        "bad byte must not propagate Err from next_raw_event"
    );
}

// ---------------------------------------------------------------------------
// try_next_raw_event: non-blocking path
// ---------------------------------------------------------------------------

#[test]
fn try_next_raw_event_returns_buffered_event() {
    let mut src = RawEventSource::new();
    src.buf.extend_from_slice(b"q");
    let ev = src.try_next_raw_event().expect("must not error");
    assert!(
        ev.is_some(),
        "pre-buffered event must be returned immediately"
    );
}

#[test]
fn try_next_raw_event_returns_none_when_buffer_empty() {
    let mut src = RawEventSource::new();
    // poll(fd=0, timeout=0) returns immediately when stdin has no data.
    let ev = src.try_next_raw_event().expect("must not error");
    assert!(ev.is_none());
}

// ---------------------------------------------------------------------------
// fill_with / PollReader: mock-based tests for #456 (exact-4096 freeze)
// ---------------------------------------------------------------------------

/// Mock [`PollReader`] that returns pre-configured poll and read results
/// from front-to-back queues.
struct MockReader {
    poll_results: VecDeque<bool>,
    read_counts: VecDeque<usize>,
}

impl PollReader for MockReader {
    fn poll(&mut self, _timeout_ms: libc::c_int) -> io::Result<bool> {
        Ok(self.poll_results.pop_front().unwrap_or(false))
    }

    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        self.read_counts
            .pop_front()
            .ok_or_else(|| io::Error::other("MockReader: unexpected extra read() call"))
    }
}

#[test]
fn fill_with_exact_4096_does_not_block() {
    // Simulate an exact 4096-byte burst with no more data available.
    // The loop must use poll(0) and break, not issue a blocking read.
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, false]),
        read_counts: VecDeque::from([4096]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 4096);
    assert!(
        reader.read_counts.is_empty(),
        "no more reads should be attempted"
    );
}

#[test]
fn fill_with_partial_read_stops_without_extra_poll() {
    // A read returning less than the buffer size means no more data is
    // pending, so the loop must break without calling poll(0).
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, false]),
        read_counts: VecDeque::from([100]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 100);
    // The poll(0) result was not consumed.
    assert_eq!(
        reader.poll_results.len(),
        1,
        "poll(0) should not be consumed after partial read"
    );
}

#[test]
fn fill_with_drains_multiple_4096_bursts() {
    // Two exact 4096-byte reads, with poll(0) returning true between them.
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true, true, false]),
        read_counts: VecDeque::from([4096, 4096]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert_eq!(src.buf.len(), 8192);
    assert!(reader.read_counts.is_empty());
}

#[test]
fn fill_with_returns_early_on_poll_timeout() {
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([false]),
        read_counts: VecDeque::from([]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert!(src.buf.is_empty());
}

#[test]
fn fill_with_empty_read_is_eof() {
    let mut src = RawEventSource::new();
    let mut reader = MockReader {
        poll_results: VecDeque::from([true]),
        read_counts: VecDeque::from([0]),
    };
    src.fill_with(&mut reader, 16).expect("fill must not error");
    assert!(src.buf.is_empty());
}

// ---------------------------------------------------------------------------
// for_tty / from_tty_opener: pager-mode fd selection (#489)
// ---------------------------------------------------------------------------

#[test]
fn new_reads_from_stdin_fd() {
    let src = RawEventSource::new();
    assert_eq!(src.fd, libc::STDIN_FILENO);
    assert!(src._tty_file.is_none());
}

#[test]
fn from_tty_opener_falls_back_to_stdin_on_open_failure() {
    let src = RawEventSource::from_tty_opener(|| Err(io::Error::other("no controlling terminal")));
    assert_eq!(src.fd, libc::STDIN_FILENO);
    assert!(src._tty_file.is_none());
}

#[test]
fn from_tty_opener_reads_through_the_opened_fd() {
    // A regular file stands in for `/dev/tty`: it exercises the real fd
    // plumbing (poll/read via `self.fd`, not hardcoded fd 0) without needing
    // an actual controlling terminal in the test environment.
    let mut file = tempfile::tempfile().expect("create temp file");
    use std::io::{Seek, SeekFrom, Write};
    file.write_all(b"q").unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();

    let mut src = RawEventSource::from_tty_opener(move || Ok(file));
    assert!(src._tty_file.is_some());
    assert_ne!(src.fd, libc::STDIN_FILENO);

    let ev = src.next_raw_event().expect("must not error");
    assert!(ev.is_some(), "event must be read through the injected fd");
}
