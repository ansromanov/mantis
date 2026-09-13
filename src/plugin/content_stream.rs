//! Bounded assembly for streamed plugin-rendered content.
//!
//! A plugin can send a rendered document as indexed line chunks instead of one
//! large `set_content` action. This module validates chunk ordering, ignores
//! duplicates, tolerates out-of-order delivery, and waits for an explicit end
//! marker before declaring the document complete. It bounds individual chunks,
//! total content, line count, and chunk count so streaming does not bypass the
//! existing plugin IPC limits. An idle stream can be marked incomplete while
//! its already-rendered lines remain available to the content pane.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// Maximum serialized source text accepted in one content chunk.
pub(crate) const MAX_CONTENT_CHUNK_BYTES: usize = 1024 * 1024;
/// Maximum combined source text retained for one streamed document.
pub(crate) const MAX_STREAM_CONTENT_BYTES: usize = 64 * 1024 * 1024;
/// Maximum number of lines retained for one streamed document.
pub(crate) const MAX_STREAM_CONTENT_LINES: usize = 1_000_000;
/// Maximum chunk index accepted for one streamed document.
pub(crate) const MAX_STREAM_CHUNKS: usize = 65_536;
/// Inactivity interval after which a partial stream is kept and marked incomplete.
pub(crate) const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(10);

/// Result of accepting a chunk or terminator.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct StreamUpdate {
    /// Newly contiguous lines that can be rendered immediately.
    pub(crate) appended: Vec<String>,
    /// Whether the explicit terminator and all chunks have arrived.
    pub(crate) complete: bool,
}

/// Why a stream could not be completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StreamError {
    ChunkTooLarge,
    ContentTooLarge,
    TooManyLines,
    TooManyChunks,
    InvalidTerminator,
}

/// Assembly state for one content id.
#[derive(Debug)]
pub(crate) struct ContentStream {
    content_id: String,
    next_chunk: usize,
    pending: BTreeMap<usize, Vec<String>>,
    accepted_bytes: usize,
    accepted_lines: usize,
    total_chunks: Option<usize>,
    last_activity: Instant,
    state: StreamState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamState {
    Active,
    Complete,
    Incomplete,
}

impl ContentStream {
    /// Starts a new stream identified by `content_id`.
    pub(crate) fn new(content_id: String, now: Instant) -> Self {
        Self {
            content_id,
            next_chunk: 0,
            pending: BTreeMap::new(),
            accepted_bytes: 0,
            accepted_lines: 0,
            total_chunks: None,
            last_activity: now,
            state: StreamState::Active,
        }
    }

    pub(crate) fn content_id(&self) -> &str {
        &self.content_id
    }

    pub(crate) fn is_active(&self) -> bool {
        self.state == StreamState::Active
    }

    pub(crate) fn is_incomplete(&self) -> bool {
        self.state == StreamState::Incomplete
    }

    /// Accepts a chunk, retaining gaps until preceding chunks arrive.
    pub(crate) fn push_chunk(
        &mut self,
        index: usize,
        lines: Vec<String>,
        now: Instant,
    ) -> Result<StreamUpdate, StreamError> {
        if !self.is_active() {
            return Ok(StreamUpdate {
                appended: Vec::new(),
                complete: self.state == StreamState::Complete,
            });
        }
        if index >= MAX_STREAM_CHUNKS {
            return self.fail(StreamError::TooManyChunks);
        }
        if index < self.next_chunk || self.pending.contains_key(&index) {
            return Ok(StreamUpdate {
                appended: Vec::new(),
                complete: false,
            });
        }
        if self.total_chunks.is_some_and(|total| index >= total) {
            return self.fail(StreamError::InvalidTerminator);
        }

        let chunk_bytes = lines.iter().map(String::len).sum::<usize>();
        if chunk_bytes > MAX_CONTENT_CHUNK_BYTES {
            return self.fail(StreamError::ChunkTooLarge);
        }
        let Some(total_bytes) = self.accepted_bytes.checked_add(chunk_bytes) else {
            return self.fail(StreamError::ContentTooLarge);
        };
        if total_bytes > MAX_STREAM_CONTENT_BYTES {
            return self.fail(StreamError::ContentTooLarge);
        }
        let Some(total_lines) = self.accepted_lines.checked_add(lines.len()) else {
            return self.fail(StreamError::TooManyLines);
        };
        if total_lines > MAX_STREAM_CONTENT_LINES {
            return self.fail(StreamError::TooManyLines);
        }

        self.accepted_bytes = total_bytes;
        self.accepted_lines = total_lines;
        self.pending.insert(index, lines);
        self.last_activity = now;

        let mut appended = Vec::new();
        while let Some(mut chunk) = self.pending.remove(&self.next_chunk) {
            appended.append(&mut chunk);
            self.next_chunk += 1;
        }
        self.finish_if_ready();
        Ok(StreamUpdate {
            appended,
            complete: self.state == StreamState::Complete,
        })
    }

    /// Records the explicit end marker, which may arrive before missing chunks.
    pub(crate) fn finish(
        &mut self,
        total_chunks: usize,
        now: Instant,
    ) -> Result<StreamUpdate, StreamError> {
        if !self.is_active() {
            return Ok(StreamUpdate {
                appended: Vec::new(),
                complete: self.state == StreamState::Complete,
            });
        }
        if total_chunks > MAX_STREAM_CHUNKS
            || total_chunks < self.next_chunk
            || self.pending.keys().any(|index| *index >= total_chunks)
            || self.total_chunks.is_some_and(|old| old != total_chunks)
        {
            return self.fail(StreamError::InvalidTerminator);
        }

        self.total_chunks = Some(total_chunks);
        self.last_activity = now;
        self.finish_if_ready();
        Ok(StreamUpdate {
            appended: Vec::new(),
            complete: self.state == StreamState::Complete,
        })
    }

    /// Marks an idle stream incomplete without discarding already assembled content.
    pub(crate) fn expire(&mut self, now: Instant) -> bool {
        if self.is_active() && now.duration_since(self.last_activity) >= STREAM_IDLE_TIMEOUT {
            self.state = StreamState::Incomplete;
            self.pending.clear();
            true
        } else {
            false
        }
    }

    fn finish_if_ready(&mut self) {
        if self.total_chunks == Some(self.next_chunk) && self.pending.is_empty() {
            self.state = StreamState::Complete;
        }
    }

    fn fail<T>(&mut self, error: StreamError) -> Result<T, StreamError> {
        self.state = StreamState::Incomplete;
        self.pending.clear();
        Err(error)
    }
}
