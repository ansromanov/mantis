//! Background file loader.
//!
//! Reading a file from disk, syntax-highlighting it, rendering markdown/JSON,
//! and shelling out to `git diff` are all expensive enough to cause visible
//! input lag when done synchronously on every tree-navigation keystroke. This
//! module moves that work onto a dedicated worker thread: the main thread pushes
//! [`LoadRequest`]s onto a channel and drains [`LoadResponse`]s on each tick,
//! applying only the newest result and discarding stale ones (see
//! `App::drain_loads`).
//!
//! The pure computation lives in [`compute_file_load`] / [`compute_diff_load`]
//! so the synchronous `App::open_file` / `App::show_working_tree_diff` paths
//! (used at startup, on reload, and in tests) share exactly the same logic as
//! the worker.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use ratatui::style::Style;

use crate::file::is_binary_bytes;
use crate::highlight::Highlighter;
use crate::theme::Theme;
use crate::virtual_file::VirtualFile;
use crate::yaml_fold::{self, FoldRegion};

type Spans = Vec<Vec<(Style, String)>>;

/// Computed result of opening a file: everything that requires disk I/O or
/// CPU-bound highlighting. Applied to the `App` on the main thread by
/// `App::apply_file_load`.
pub(super) struct FileLoad {
    pub is_markdown: bool,
    pub is_json: bool,
    pub virtual_file: Option<VirtualFile>,
    pub content: Vec<String>,
    pub highlighted: Spans,
    pub markdown_lines: Spans,
    pub json_pretty_text: Vec<String>,
    pub json_pretty_lines: Spans,
    pub show_pretty_json: bool,
    pub yaml: Option<YamlLoad>,
    /// `false` when the file could not be read; `current_file` is cleared so the
    /// inline `[error: …]` message stands alone.
    pub ok: bool,
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

impl FileLoad {
    fn empty(is_markdown: bool, is_json: bool) -> Self {
        FileLoad {
            is_markdown,
            is_json,
            virtual_file: None,
            content: Vec::new(),
            highlighted: Vec::new(),
            markdown_lines: Vec::new(),
            json_pretty_text: Vec::new(),
            json_pretty_lines: Vec::new(),
            show_pretty_json: false,
            yaml: None,
            ok: true,
        }
    }
}

/// Reads `path`, detects binary/markdown/json/yaml, and produces the rendered
/// content. Plain files use a memory-mapped [`VirtualFile`] (highlighted lazily
/// in the UI); markdown/JSON/YAML are read fully and rendered here. This is the
/// single source of truth shared by the synchronous and worker code paths.
pub(super) fn compute_file_load(path: &Path, theme: &Theme, hl: &Highlighter) -> FileLoad {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let is_markdown = matches!(ext, "md" | "markdown");
    let is_json = ext == "json";
    let is_yaml = matches!(ext, "yaml" | "yml");

    // Try memory-mapped virtual file first (lazy, no full content in memory).
    // Markdown, JSON, and YAML are excluded: they need full content for
    // rendering/validation.
    if !is_markdown && !is_json && !is_yaml {
        if let Some(vf) = VirtualFile::open(path) {
            let mut load = FileLoad::empty(false, false);
            load.virtual_file = Some(vf);
            return load;
        }
    }

    let mut load = FileLoad::empty(is_markdown, is_json);

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
        return load;
    }
    let s = match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            load.content = vec!["[binary file]".into()];
            return load;
        }
    };
    load.content = s.lines().map(|l| l.to_owned()).collect();
    if load.content.is_empty() {
        load.content = vec!["[empty file]".into()];
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
    if is_markdown {
        load.markdown_lines = crate::markdown::render(&s, theme);
    }
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

/// Runs `git diff HEAD` for `path` and parses it into renderable diff state.
pub(super) fn compute_diff_load(root: &Path, path: &Path, theme: &Theme) -> DiffLoad {
    let lines = crate::git::working_tree_diff(root, path);
    let rel = path.strip_prefix(root).unwrap_or(path);
    let highlighted = lines
        .iter()
        .map(|l| vec![(super::diff_line_style(l, theme), l.clone())])
        .collect();
    let diff_rows = crate::diff::parse_side_by_side(&lines);
    DiffLoad {
        content_title: format!(" working diff — {} ", rel.display()),
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
    },
    /// Rebuild the worker's highlighter/theme after a theme change.
    SetTheme(Box<Theme>),
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
    pub fn new(theme: &Theme) -> Self {
        let (req_tx, req_rx) = std::sync::mpsc::channel::<LoadRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<LoadResponse>();
        let mut theme = theme.clone();
        let handle = std::thread::spawn(move || {
            let mut hl = Highlighter::new(&theme.syntax);
            while let Ok(req) = req_rx.recv() {
                match req {
                    LoadRequest::Shutdown => break,
                    LoadRequest::SetTheme(t) => {
                        theme = *t;
                        hl = Highlighter::new(&theme.syntax);
                    }
                    LoadRequest::File { seq, path } => {
                        let load = Box::new(compute_file_load(&path, &theme, &hl));
                        if res_tx.send(LoadResponse::File { seq, path, load }).is_err() {
                            break;
                        }
                    }
                    LoadRequest::Diff { seq, root, path } => {
                        let load = Box::new(compute_diff_load(&root, &path, &theme));
                        if res_tx.send(LoadResponse::Diff { seq, path, load }).is_err() {
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
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn hl() -> Highlighter {
        Highlighter::new("base16-ocean.dark")
    }

    #[test]
    fn plain_file_uses_virtual_file() {
        let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        use std::io::Write;
        f.write_all(b"fn main() {}\n").unwrap();
        let load = compute_file_load(f.path(), &Theme::default(), &hl());
        assert!(load.ok);
        assert!(load.virtual_file.is_some());
        // Plain files are highlighted lazily in the UI, not here.
        assert!(load.highlighted.is_empty());
        assert!(!load.is_markdown && !load.is_json);
    }

    #[test]
    fn markdown_renders_lines() {
        let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
        use std::io::Write;
        f.write_all(b"# Title\n\nbody\n").unwrap();
        let load = compute_file_load(f.path(), &Theme::default(), &hl());
        assert!(load.is_markdown);
        assert!(load.virtual_file.is_none());
        assert!(!load.markdown_lines.is_empty());
    }

    #[test]
    fn json_produces_pretty_view() {
        let mut f = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        use std::io::Write;
        f.write_all(br#"{"a":1,"b":[2,3]}"#).unwrap();
        let load = compute_file_load(f.path(), &Theme::default(), &hl());
        assert!(load.is_json);
        assert!(load.show_pretty_json);
        assert!(!load.json_pretty_text.is_empty());
        assert!(!load.json_pretty_lines.is_empty());
    }

    #[test]
    fn yaml_detects_folds_and_anchors() {
        let mut f = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();
        use std::io::Write;
        f.write_all(b"root: &a\n  key: val\nref: *a\n").unwrap();
        let load = compute_file_load(f.path(), &Theme::default(), &hl());
        let yaml = load.yaml.expect("yaml state");
        assert_eq!(yaml.anchor_count, 1);
        assert_eq!(yaml.alias_count, 1);
        assert!(!yaml.fold_regions.is_empty());
    }

    #[test]
    fn missing_file_is_not_ok() {
        let load = compute_file_load(
            std::path::Path::new("/no/such/file.txt"),
            &Theme::default(),
            &hl(),
        );
        assert!(!load.ok);
        assert!(load.content[0].starts_with("[error:"));
    }

    #[test]
    fn empty_file_message() {
        let mut f = tempfile::NamedTempFile::with_suffix(".md").unwrap();
        use std::io::Write;
        f.write_all(b"").unwrap();
        let load = compute_file_load(f.path(), &Theme::default(), &hl());
        assert_eq!(load.content, vec!["[empty file]".to_string()]);
    }

    #[test]
    fn worker_round_trip_returns_matching_seq() {
        let mut f = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        use std::io::Write;
        f.write_all(b"let x = 1;\n").unwrap();
        let loader = Loader::new(&Theme::default());
        loader.request(LoadRequest::File {
            seq: 7,
            path: f.path().to_path_buf(),
        });
        let resp = loader
            .rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("worker response");
        match resp {
            LoadResponse::File { seq, load, .. } => {
                assert_eq!(seq, 7);
                assert!(load.ok);
            }
            _ => panic!("expected File response"),
        }
    }
}
