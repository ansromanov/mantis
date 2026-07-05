//! Background file loader.
//!
//! Reading a file from disk, syntax-highlighting it, rendering JSON,
//! and shelling out to `git diff` are all expensive enough to cause visible
//! input lag when done synchronously on every tree-navigation keystroke. This
//! module moves that work onto a dedicated worker thread: the main thread pushes
//! [`LoadRequest`]s onto a channel and drains [`LoadResponse`]s on each tick,
//! applying only the newest result and discarding stale ones (see
//! `App::drain_loads`).
//!
//! The pure computation lives in [`compute_file_load`] / [`compute_diff_load`]
//! so the synchronous `App::open_file` / `App::show_working_tree_diff` paths
//! (used at startup, on reload) share exactly the same logic as the worker.
//!
//! `compute_diff_load` accepts a [`crate::app::DiffMode`] parameter to choose
//! between all-changes, staged, and unstaged diff variants.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;

use ratatui::style::Style;

use crate::app::DiffMode;
use crate::file::{detect_encoding_prefix, detect_line_ending, is_binary_bytes};
use crate::fold::FoldRegion;
use crate::git::GitStatus;
use crate::highlight::Highlighter;
use crate::plugin::ExtraSyntax;
use crate::theme::Theme;
use crate::virtual_file::VirtualFile;
use crate::yaml_fold;

type Spans = Vec<Vec<(Style, String)>>;

/// Computed result of opening a file: everything that requires disk I/O or
/// CPU-bound highlighting. Applied to the `App` on the main thread by
/// `App::apply_file_load`.
pub(super) struct FileLoad {
    pub is_json: bool,
    pub virtual_file: Option<VirtualFile>,
    pub content: Vec<String>,
    pub highlighted: Spans,
    pub json_pretty_text: Vec<String>,
    pub json_pretty_lines: Spans,
    pub show_pretty_json: bool,
    pub yaml: Option<YamlLoad>,
    /// `true` when the file size exceeds the configured `prettify_size_limit`
    /// and JSON/YAML pretty-printing / fold detection was skipped.
    pub prettify_size_limit_exceeded: bool,
    /// `false` when the file could not be read; `current_file` is cleared so the
    /// inline `[error: …]` message stands alone.
    pub ok: bool,
    /// Detected text encoding, e.g. `"UTF-8"`, `"ASCII"`, `"UTF-8 BOM"`.
    /// `None` when the encoding could not be determined (binary, error).
    pub encoding: Option<String>,
    /// Detected line-ending style: `"LF"`, `"CRLF"`, `"CR"`, or `"mixed"`.
    /// `None` for single-line or empty files.
    pub line_ending: Option<String>,
    /// Detected syntax/language name from syntect, e.g. `"Rust"` or `"Python"`.
    /// `None` for plain text or when syntect has no match. Populated by the
    /// worker thread so the main thread never calls `find_syntax_for_file`.
    pub syntax_name: Option<String>,
}

/// YAML-specific derived state, computed only for `.yaml`/`.yml` files.
pub(super) struct YamlLoad {
    pub fold_regions: Vec<FoldRegion>,
    pub error: Option<String>,
    pub anchor_count: usize,
    pub alias_count: usize,
}

/// Computed result of a working-tree diff.
pub(super) struct DiffLoad {
    pub content: Vec<String>,
    pub highlighted: Spans,
    pub diff_rows: Vec<crate::diff::DiffRow>,
    pub content_title: String,
}

/// Computed result of a git status scan (status map + repo info).
pub(super) struct GitStatusLoad {
    pub status_map: HashMap<PathBuf, GitStatus>,
    pub info: Option<crate::git::GitRepoInfo>,
}

impl FileLoad {
    fn empty(is_json: bool) -> Self {
        FileLoad {
            is_json,
            virtual_file: None,
            content: Vec::new(),
            highlighted: Vec::new(),
            json_pretty_text: Vec::new(),
            json_pretty_lines: Vec::new(),
            show_pretty_json: false,
            yaml: None,
            prettify_size_limit_exceeded: false,
            ok: true,
            encoding: None,
            line_ending: None,
            syntax_name: None,
        }
    }
}

/// Reads `path`, detects binary/json/yaml, and produces the rendered content.
/// Plain files use a memory-mapped [`VirtualFile`] (highlighted lazily in the
/// UI); JSON/YAML are read fully and rendered here. This is the single source
/// of truth shared by the synchronous and worker code paths.
///
/// `prettify_size_limit` is the maximum file size (bytes) for which
/// JSON/YAML pretty-printing and fold-region detection are performed. Files
/// exceeding this threshold are loaded via the mmap path and shown as raw
/// content, with `prettify_size_limit_exceeded` set on the returned structure.
pub(super) fn compute_file_load(
    path: &Path,
    hl: &Highlighter,
    prettify_size_limit: usize,
) -> FileLoad {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_json = ext == "json";
    let is_yaml = matches!(ext, "yaml" | "yml");

    // Check whether this JSON/YAML file exceeds the pretty-print size limit.
    let too_large = if is_json || is_yaml {
        std::fs::metadata(path)
            .ok()
            .is_some_and(|meta| meta.len() > prettify_size_limit as u64)
    } else {
        false
    };

    // Try memory-mapped virtual file first (lazy, no full content in memory).
    // JSON and YAML are excluded (they need full content for rendering) unless
    // the file exceeds the prettify size limit.
    if (!is_json && !is_yaml) || too_large {
        if let Some(vf) = VirtualFile::open(path) {
            let mut load = FileLoad::empty(is_json);
            let raw = vf.raw_bytes();
            // VirtualFile::open already verified valid UTF-8, so skip the full
            // re-validation pass; only the BOM/ASCII prefix check is needed.
            load.encoding = Some(detect_encoding_prefix(raw).unwrap_or("UTF-8").to_string());
            load.line_ending = detect_line_ending(raw).map(|s| s.to_string());
            load.syntax_name = hl.syntax_name(path);
            load.virtual_file = Some(vf);
            load.prettify_size_limit_exceeded = too_large;
            return load;
        }
    }

    let mut load = FileLoad::empty(is_json);

    // Fallback: read the file into memory (small files, binary check, etc.).
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            load.content = vec![format!("[error: {}]", e)];
            load.ok = false;
            return load;
        }
    };
    if is_binary_bytes(&bytes) {
        load.content = vec!["[binary file]".into()];
        load.encoding = Some("BINARY".into());
        return load;
    }
    // Detect line endings and BOM/ASCII classification before consuming bytes.
    // Full UTF-8 validity is confirmed by String::from_utf8 below, avoiding a
    // double validation pass.
    load.line_ending = detect_line_ending(&bytes).map(|s| s.to_string());
    let enc_prefix = detect_encoding_prefix(&bytes);
    let s = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            load.content = vec!["[binary file]".into()];
            load.encoding = Some("BINARY".into());
            return load;
        }
    };
    // String::from_utf8 succeeded, so the bytes are valid UTF-8; use the
    // prefix result or fall back to "UTF-8".
    load.encoding = Some(enc_prefix.unwrap_or("UTF-8").to_string());
    // str::lines() does not treat bare \r as a line terminator, so CR-only
    // files would render as a single line with embedded \r control characters.
    // Normalize CR and CRLF to LF before splitting.
    let s = if s.contains('\r') {
        s.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        s
    };
    load.content = s.lines().map(|l| l.to_owned()).collect();
    if load.content.is_empty() {
        load.content = vec!["[empty file]".into()];
        return load;
    }

    // Skip YAML/JSON processing when the file exceeds the prettify size limit;
    // the mmap path above should have handled this, but guard the fallback too.
    if too_large {
        load.prettify_size_limit_exceeded = true;
        load.highlighted = hl.highlight(path, &load.content);
        load.syntax_name = hl.syntax_name(path);
        return load;
    }

    if is_yaml {
        let fold_regions = yaml_fold::detect_fold_regions(&load.content);
        let error = serde_yaml::from_str::<serde_yaml::Value>(&s)
            .err()
            .map(|e| e.to_string());
        let (anchor_count, alias_count) = yaml_fold::count_anchors_aliases(&load.content);
        load.yaml = Some(YamlLoad {
            fold_regions,
            error,
            anchor_count,
            alias_count,
        });
    }
    load.highlighted = hl.highlight(path, &load.content);
    load.syntax_name = hl.syntax_name(path);
    if is_json {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                let pretty_lines: Vec<String> = pretty.lines().map(|l| l.to_owned()).collect();
                load.json_pretty_lines = hl.highlight(path, &pretty_lines);
                load.json_pretty_text = pretty_lines;
                load.show_pretty_json = true;
            }
        }
    }
    load
}

/// Runs `repo_status` + `repo_info` for `root` off the UI thread.
pub(super) fn compute_git_status_load(
    root: &Path,
    include_untracked: bool,
    include_ignored: bool,
) -> GitStatusLoad {
    let status_map = crate::git::repo_status(root, include_untracked, include_ignored);
    let info = crate::git::repo_info(root);
    GitStatusLoad { status_map, info }
}

/// Runs the appropriate `git diff` variant for `path` and parses it into
/// renderable diff state. The `diff_mode` parameter selects between all changes
/// vs HEAD, staged changes only, or unstaged changes only.
pub(super) fn compute_diff_load(
    root: &Path,
    path: &Path,
    theme: &Theme,
    diff_mode: DiffMode,
) -> DiffLoad {
    let lines = match diff_mode {
        DiffMode::All => crate::git::working_tree_diff(root, path),
        DiffMode::Staged => crate::git::staged_diff(root, path),
        DiffMode::Unstaged => crate::git::unstaged_diff(root, path),
    };
    let rel = path.strip_prefix(root).unwrap_or(path);
    let highlighted = lines
        .iter()
        .map(|l| vec![(super::diff_line_style(l, theme), l.clone())])
        .collect();
    let diff_rows = crate::diff::parse_side_by_side(&lines);
    DiffLoad {
        content_title: format!(" working diff — {} [{}] ", rel.display(), diff_mode.label()),
        highlighted,
        diff_rows,
        content: lines,
    }
}

/// A request sent to the worker thread. Each load carries a monotonically
/// increasing `seq` so the main thread can discard results that have been
/// superseded by a newer navigation.
pub(super) enum LoadRequest {
    File {
        seq: u64,
        path: PathBuf,
    },
    Diff {
        seq: u64,
        root: PathBuf,
        path: PathBuf,
        diff_mode: DiffMode,
    },
    /// Fetch git status (`repo_status` + `repo_info`) for `root`.
    GitStatus {
        seq: u64,
        root: PathBuf,
        include_untracked: bool,
        include_ignored: bool,
    },
    /// Rebuild the worker's highlighter/theme after a theme change.
    SetTheme(Box<Theme>),
    /// Rebuild the worker's highlighter with updated syntax definitions.
    SetExtraSyntaxes(Vec<ExtraSyntax>),
    /// Test-only: echoed back once the worker has drained every request
    /// queued ahead of it, giving `App::pump_loads` a deterministic
    /// completion signal instead of a silence-based timeout.
    #[cfg(test)]
    Barrier(u64),
    /// Stop the worker (sent on `Loader` drop).
    Shutdown,
}

/// A result produced by the worker thread, tagged with the originating `seq`.
pub(super) enum LoadResponse {
    File {
        seq: u64,
        path: PathBuf,
        load: Box<FileLoad>,
    },
    Diff {
        seq: u64,
        path: PathBuf,
        load: Box<DiffLoad>,
    },
    GitStatus {
        seq: u64,
        root: PathBuf,
        load: Box<GitStatusLoad>,
    },
    /// Test-only: reply to a [`LoadRequest::Barrier`].
    #[cfg(test)]
    Barrier(u64),
}

/// Owns the worker thread and the request/response channels. The worker keeps
/// its own [`Highlighter`] and [`Theme`] so highlighting never touches the
/// main thread's copy.
pub(super) struct Loader {
    tx: Sender<LoadRequest>,
    pub rx: Receiver<LoadResponse>,
    handle: Option<JoinHandle<()>>,
}

impl Loader {
    pub fn new(
        theme: &Theme,
        extra_syntaxes: Vec<ExtraSyntax>,
        prettify_size_limit: usize,
    ) -> Self {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<LoadRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<LoadResponse>();
        let mut extra_for_thread = Arc::new(extra_syntaxes);
        let mut theme = theme.clone();
        let handle = std::thread::spawn(move || {
            let mut hl = Highlighter::with_extra_syntaxes(&theme.syntax, &extra_for_thread);
            while let Ok(req) = req_rx.recv() {
                match req {
                    LoadRequest::Shutdown => break,
                    LoadRequest::SetTheme(t) => {
                        theme = *t;
                        hl = Highlighter::with_extra_syntaxes(&theme.syntax, &extra_for_thread);
                    }
                    LoadRequest::SetExtraSyntaxes(extra) => {
                        extra_for_thread = Arc::new(extra);
                        hl = Highlighter::with_extra_syntaxes(&theme.syntax, &extra_for_thread);
                    }
                    LoadRequest::File { seq, path } => {
                        let load = Box::new(compute_file_load(&path, &hl, prettify_size_limit));
                        if res_tx.send(LoadResponse::File { seq, path, load }).is_err() {
                            break;
                        }
                    }
                    LoadRequest::GitStatus {
                        seq,
                        root,
                        include_untracked,
                        include_ignored,
                    } => {
                        let load = Box::new(compute_git_status_load(
                            &root,
                            include_untracked,
                            include_ignored,
                        ));
                        if res_tx
                            .send(LoadResponse::GitStatus {
                                seq,
                                root: root.clone(),
                                load,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    LoadRequest::Diff {
                        seq,
                        root,
                        path,
                        diff_mode,
                    } => {
                        let load = Box::new(compute_diff_load(&root, &path, &theme, diff_mode));
                        if res_tx.send(LoadResponse::Diff { seq, path, load }).is_err() {
                            break;
                        }
                    }
                    #[cfg(test)]
                    LoadRequest::Barrier(token) => {
                        if res_tx.send(LoadResponse::Barrier(token)).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Loader {
            tx: req_tx,
            rx: res_rx,
            handle: Some(handle),
        }
    }

    /// Queues a request for the worker. Best-effort: if the worker has gone away
    /// the send is silently dropped (the app is shutting down).
    pub fn request(&self, req: LoadRequest) {
        let _ = self.tx.send(req);
    }
}

impl Drop for Loader {
    fn drop(&mut self) {
        let _ = self.tx.send(LoadRequest::Shutdown);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

#[cfg(test)]
#[path = "loader_test.rs"]
mod tests;
