use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

use crate::file::is_binary_bytes;

/// A file whose content is memory-mapped and accessed lazily via a line index.
///
/// Instead of reading the entire file into a `Vec<String>`, we memory-map it and
/// build a lightweight offset table (one `usize` per line). Individual lines are
/// extracted on demand without allocating per-line strings until needed.
pub struct VirtualFile {
    mmap: Mmap,
    /// Byte offset of each line's first character. Length equals `count`.
    line_offsets: Vec<usize>,
    /// Total number of lines (trailing empty line from final `\n` is excluded).
    count: usize,
}

impl VirtualFile {
    /// Opens `path`, memory-maps it, and builds the line offset index.
    /// Returns `None` if the file cannot be opened, is empty, is binary, or
    /// is not valid UTF-8.
    pub fn open(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        let mmap = unsafe { Mmap::map(&file).ok()? };

        // Reject binary and non-UTF-8 files.
        if is_binary_bytes(&mmap) {
            return None;
        }
        if std::str::from_utf8(&mmap).is_err() {
            return None;
        }

        let line_offsets = build_line_offsets(&mmap);
        let count = line_offsets.len();

        if count == 0 {
            return None;
        }

        Some(VirtualFile {
            mmap,
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
        let start = self.line_offsets[index];
        let end = if index + 1 < self.count {
            self.line_offsets[index + 1]
        } else {
            self.mmap.len()
        };
        let slice = &self.mmap[start..end];
        let s = std::str::from_utf8(slice).ok()?;
        let no_lf = s.strip_suffix('\n').unwrap_or(s);
        Some(no_lf.strip_suffix('\r').unwrap_or(no_lf))
    }

    /// Returns the display width (in terminal columns) of line `index`.
    pub fn line_width(&self, index: usize) -> Option<usize> {
        self.line_text(index)
            .map(unicode_width::UnicodeWidthStr::width)
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
