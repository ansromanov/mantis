//! Lazily-indexed file backing for large files, with safe owned storage for
//! small-to-medium files.
//!
//! `VirtualFile` opens a file and builds only a per-line byte-offset table
//! instead of reading the whole thing into a `Vec<String>`. Lines are sliced
//! out of the backing store on demand, so opening a huge file is cheap and the
//! content pane highlights only the visible window. It exposes the line count
//! and per-line text accessors the content-query layer expects, and uses
//! `is_binary_bytes` to refuse binary data.
//!
//! Files **≤ 16 MB** are read into an owned `Vec<u8>` so that external
//! truncation cannot SIGBUS the process. Files **> 16 MB** use `memmap2` for
//! zero-copy access; external truncation of a mapped file can still raise
//! SIGBUS (documented trade-off for large-file performance). This is the
//! default content source for ordinary files; small or special cases (errors,
//! binaries, diffs, rendered markdown) use other in-memory representations
//! instead.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use memmap2::Mmap;

use crate::file::is_binary_bytes;

/// Files at or below this threshold are fully read into memory so that
/// external truncation cannot crash the process with SIGBUS.
/// Files larger than this threshold use `mmap` (SIGBUS risk is documented
/// and accepted for large-file performance).
const MMAP_THRESHOLD: u64 = 16 * 1024 * 1024;

/// Backing storage for a [`VirtualFile`].
///
/// Small files are owned as a `Vec<u8>` to prevent SIGBUS on external
/// truncation. Large files use a memory map for zero-copy access.
enum VirtualFileData {
    Mapped(Mmap),
    Owned(Vec<u8>),
}

impl VirtualFileData {
    fn as_bytes(&self) -> &[u8] {
        match self {
            VirtualFileData::Mapped(m) => m,
            VirtualFileData::Owned(v) => v,
        }
    }
}

/// A file whose content is accessed lazily via a line index.
///
/// Files ≤ `MMAP_THRESHOLD` (16 MB) are stored in an owned `Vec<u8>` so
/// external truncation cannot SIGBUS the process. Larger files use a memory
/// map for zero-copy access. In both cases, a lightweight offset table (one
/// `usize` per line) is built at open time so individual lines can be
/// extracted on demand without allocating per-line strings.
pub struct VirtualFile {
    data: VirtualFileData,
    /// Byte offset of each line's first character. Length equals `count`.
    line_offsets: Vec<usize>,
    /// Total number of lines (trailing empty line from final `\n` is excluded).
    count: usize,
}

impl VirtualFile {
    /// Opens `path`, reads or memory-maps the content based on file size,
    /// and builds the line offset index.
    ///
    /// Returns `None` if the file cannot be opened, is empty, is binary, or
    /// is not valid UTF-8.
    pub fn open(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        let len = metadata.len();

        let data: VirtualFileData = if len == 0 {
            return None;
        } else if len <= MMAP_THRESHOLD {
            // Read into owned memory: safe against external truncation.
            let mut buf = Vec::with_capacity(len as usize);
            file.read_to_end(&mut buf).ok()?;
            VirtualFileData::Owned(buf)
        } else {
            // Memory-map large files (SIGBUS risk on external truncation).
            VirtualFileData::Mapped(unsafe { Mmap::map(&file).ok()? })
        };

        // Reject binary and non-UTF-8 files.
        if is_binary_bytes(data.as_bytes()) {
            return None;
        }
        if std::str::from_utf8(data.as_bytes()).is_err() {
            return None;
        }

        let line_offsets = build_line_offsets(data.as_bytes());
        let count = line_offsets.len();

        if count == 0 {
            return None;
        }

        Some(VirtualFile {
            data,
            line_offsets,
            count,
        })
    }

    pub fn line_count(&self) -> usize {
        self.count
    }

    /// Returns the text of line `index` (0-based), with the trailing newline
    /// stripped. Returns `None` if the index is out of bounds.
    pub fn line_text(&self, index: usize) -> Option<&str> {
        if index >= self.count {
            return None;
        }
        let bytes = self.data.as_bytes();
        let start = self.line_offsets[index];
        let end = if index + 1 < self.count {
            self.line_offsets[index + 1]
        } else {
            bytes.len()
        };
        let slice = &bytes[start..end];
        let s = std::str::from_utf8(slice).ok()?;
        let no_lf = s.strip_suffix('\n').unwrap_or(s);
        Some(no_lf.strip_suffix('\r').unwrap_or(no_lf))
    }

    /// Returns the display width (in terminal columns) of line `index`.
    pub fn line_width(&self, index: usize) -> Option<usize> {
        self.line_text(index)
            .map(unicode_width::UnicodeWidthStr::width)
    }

    /// Returns the raw bytes of the file (owned or memory-mapped). Used by
    /// encoding and line-ending detection in the file loader.
    pub fn raw_bytes(&self) -> &[u8] {
        self.data.as_bytes()
    }
}

/// Scans `mmap` for `\n` bytes and returns the start offset of each line.
fn build_line_offsets(mmap: &[u8]) -> Vec<usize> {
    let mut offsets = Vec::new();
    offsets.push(0);
    for (i, &b) in mmap.iter().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    // If the file ends with `\n`, the last offset points one past the end and
    // would represent an empty trailing line — drop it.
    if offsets.last().is_some_and(|&o| o == mmap.len()) {
        offsets.pop();
    }
    offsets
}

#[cfg(test)]
#[path = "virtual_file_test.rs"]
mod tests;
