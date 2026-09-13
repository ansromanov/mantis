use super::*;

fn now() -> Instant {
    Instant::now()
}

#[test]
fn appends_chunks_in_order_when_they_arrive_out_of_order() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);

    assert_eq!(
        stream.push_chunk(2, vec!["c".into()], time).unwrap().appended,
        Vec::<String>::new()
    );
    assert_eq!(
        stream.push_chunk(0, vec!["a".into()], time).unwrap().appended,
        vec!["a"]
    );
    assert_eq!(
        stream.push_chunk(1, vec!["b".into()], time).unwrap().appended,
        vec!["b", "c"]
    );
}

#[test]
fn ignores_duplicate_chunks_without_duplicating_output() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);
    assert_eq!(
        stream.push_chunk(0, vec!["a".into()], time).unwrap().appended,
        vec!["a"]
    );
    assert!(stream
        .push_chunk(0, vec!["duplicate".into()], time)
        .unwrap()
        .appended
        .is_empty());
}

#[test]
fn terminator_completes_only_after_every_chunk_arrives() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);
    assert!(!stream.finish(2, time).unwrap().complete);
    assert!(!stream.push_chunk(1, vec!["b".into()], time).unwrap().complete);
    assert!(stream.push_chunk(0, vec!["a".into()], time).unwrap().complete);
    assert!(!stream.is_active());
}

#[test]
fn rejects_conflicting_or_incomplete_terminators() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);
    stream.push_chunk(0, vec!["a".into()], time).unwrap();
    assert_eq!(stream.finish(0, time), Err(StreamError::InvalidTerminator));
    assert!(stream.is_incomplete());
}

#[test]
fn idle_stream_expires_and_preserves_active_output_state() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);
    stream.push_chunk(0, vec!["partial".into()], time).unwrap();
    assert!(!stream.expire(time + STREAM_IDLE_TIMEOUT - Duration::from_millis(1)));
    assert!(stream.expire(time + STREAM_IDLE_TIMEOUT));
    assert!(stream.is_incomplete());
}

#[test]
fn enforces_chunk_size_total_content_and_line_caps() {
    let time = now();
    let mut large_chunk = ContentStream::new("large-chunk".into(), time);
    assert_eq!(
        large_chunk.push_chunk(
            0,
            vec!["x".repeat(MAX_CONTENT_CHUNK_BYTES + 1)],
            time
        ),
        Err(StreamError::ChunkTooLarge)
    );

    let mut content_cap = ContentStream::new("content-cap".into(), time);
    for index in 0..MAX_STREAM_CONTENT_BYTES / MAX_CONTENT_CHUNK_BYTES {
        content_cap
            .push_chunk(
                index,
                vec!["x".repeat(MAX_CONTENT_CHUNK_BYTES)],
                time,
            )
            .unwrap();
    }
    assert_eq!(
        content_cap.push_chunk(
            MAX_STREAM_CONTENT_BYTES / MAX_CONTENT_CHUNK_BYTES,
            vec!["x".into()],
            time
        ),
        Err(StreamError::ContentTooLarge)
    );

    let mut line_cap = ContentStream::new("line-cap".into(), time);
    assert_eq!(
        line_cap.push_chunk(
            0,
            vec![String::new(); MAX_STREAM_CONTENT_LINES + 1],
            time
        ),
        Err(StreamError::TooManyLines)
    );
}

#[test]
fn accepts_a_document_larger_than_the_single_message_limit() {
    let time = now();
    let mut stream = ContentStream::new("large-document".into(), time);
    let line = "x".repeat(MAX_CONTENT_CHUNK_BYTES);
    let mut delivered = 0;
    for index in 0..17 {
        delivered += stream
            .push_chunk(index, vec![line.clone()], time)
            .unwrap()
            .appended
            .iter()
            .map(String::len)
            .sum::<usize>();
    }
    assert!(delivered > crate::plugin::process::MAX_LINE_LEN);
    assert!(stream.finish(17, time).unwrap().complete);
}

#[test]
fn rejects_chunks_outside_the_terminator_range() {
    let time = now();
    let mut stream = ContentStream::new("render-1".into(), time);
    stream.finish(1, time).unwrap();
    assert_eq!(
        stream.push_chunk(1, vec!["out of range".into()], time),
        Err(StreamError::InvalidTerminator)
    );
    assert!(stream.is_incomplete());
}

#[cfg(test)]
#[path = "content_stream_test.rs"]
mod tests;
