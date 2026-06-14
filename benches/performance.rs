// Run: cargo bench [-- "filter"]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tree_viewer::highlight::Highlighter;
use tree_viewer::markdown;
use tree_viewer::search::SearchState;
use tree_viewer::theme::Theme;
use tree_viewer::tree::{build_visible, collect_all_files};
use tree_viewer::virtual_file::VirtualFile;

// ---------------------------------------------------------------------------
// Counter for unique temp dir names
// ---------------------------------------------------------------------------

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn fixture_dir(label: &str) -> PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("tv_bench_{}_{}", label, n));
    if dir.exists() {
        fs::remove_dir_all(&dir).unwrap();
    }
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Fixture generators
// ---------------------------------------------------------------------------

/// `count` files (~50 bytes each) in a flat directory.
fn generate_many_files(dir: &Path, count: usize) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..count {
        let content =
            format!("// file {i}\npub fn example() -> i32 {{\n    let x = {i};\n    x\n}}\n");
        fs::write(dir.join(format!("f{i:05}.rs")), &content).unwrap();
    }
}

/// A single `.rs` file with `line_count` lines of realistic-ish Rust code.
fn generate_large_rs_file(path: &Path, line_count: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut content = String::with_capacity(line_count * 50);
    for i in 0..line_count {
        match i % 5 {
            0 => content.push_str(&format!("// line {i} — a comment\n")),
            1 => content.push_str(&format!("pub fn func_{i}() -> i32 {{ {i} }}\n")),
            2 => content.push_str(&format!("let x_{i} = \"hello world {i}\";\n")),
            3 => content.push_str(&format!("if x_{i} > 0 {{ println!(\"{{}}\", x_{i}); }}\n")),
            _ => content.push_str(&format!("/* multi\nline\ncomment {i} */\n")),
        }
    }
    fs::write(path, content).unwrap();
}

/// A deep directory tree: d0/d1/d2/.../f{i}.rs at each level.
fn generate_deep_tree(dir: &Path, depth: usize) {
    let mut current = dir.to_path_buf();
    for i in 0..depth {
        current = current.join(format!("d{i}"));
        fs::create_dir_all(&current).unwrap();
        fs::write(current.join(format!("f{i}.rs")), "fn f() {}\n").unwrap();
    }
}

/// A large markdown file with headings, tables, code blocks, lists, etc.
/// Produces a file with exactly `line_count` lines (counted by `\n`).
fn generate_large_markdown(path: &Path, line_count: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut content = String::with_capacity(line_count * 60);
    content.push_str("# Large Benchmark Document\n\n");
    let mut lines_written = 2usize;
    let mut i = 0usize;
    while lines_written < line_count {
        let chunk = match i % 10 {
            0 => format!("## Section {i}\n\n"),
            1 => "Paragraph with **bold** and *italic* text and `code`.\n\n".to_string(),
            2 => "- list item 1\n- list item 2\n- list item 3\n\n".to_string(),
            3 => "> Block quote with some text in it.\n\n".to_string(),
            4 => "```rust\nfn hello() { println!(\"world\"); }\n```\n\n".to_string(),
            5 => format!(
                "| Col A | Col B | Col C |\n|-------|-------|-------|\n| {i}A    | \
                     {i}B    | {i}C    |\n\n"
            ),
            6 => "---\n\n".to_string(),
            _ => format!("Regular paragraph with some content at line {i}.\n\n"),
        };
        let chunk_lines = chunk.chars().filter(|&c| c == '\n').count();
        if lines_written + chunk_lines > line_count {
            break;
        }
        content.push_str(&chunk);
        lines_written += chunk_lines;
        i += 1;
    }
    fs::write(path, content).unwrap();
}

/// Generates searchable text files with `count` files each having `lines` lines.
fn generate_search_files(dir: &Path, count: usize, lines: usize) {
    fs::create_dir_all(dir).unwrap();
    for i in 0..count {
        let mut content = String::with_capacity(lines * 40);
        for j in 0..lines {
            content.push_str(&format!(
                "line {j} of file {i}: the quick brown fox jumps over the lazy dog\n"
            ));
        }
        fs::write(dir.join(format!("s{i:05}.txt")), &content).unwrap();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively visit every directory so the whole tree is visible.
fn expand_all(root: &Path) -> HashSet<PathBuf> {
    let mut expanded = HashSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if expanded.insert(dir.clone()) {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                        stack.push(entry.path());
                    }
                }
            }
        }
    }
    expanded
}

fn highlighter() -> Highlighter {
    Highlighter::new("base16-ocean.dark")
}

fn theme() -> Theme {
    Theme::default()
}

// ---------------------------------------------------------------------------
// Benchmark: tree walk — build_visible + collect_all_files
// ---------------------------------------------------------------------------

fn bench_tree_walk(c: &mut Criterion) {
    let mut group = c.benchmark_group("tree_walk");

    for &count in &[100usize, 1_000] {
        let dir = fixture_dir("tree_flat");
        generate_many_files(&dir, count);
        let root = dir.canonicalize().unwrap();
        let expanded = expand_all(&root);

        group.bench_with_input(
            BenchmarkId::new("build_visible/flat", count),
            &(&root, &expanded),
            |b, (root, expanded)| {
                let deleted: HashSet<PathBuf> = HashSet::new();
                b.iter(|| black_box(build_visible(root, expanded, false, true, &deleted)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("collect_all_files/flat", count),
            &root,
            |b, root| b.iter(|| black_box(collect_all_files(root, false, true))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    for &depth in &[10usize, 50] {
        let dir = fixture_dir("tree_deep");
        generate_deep_tree(&dir, depth);
        let root = dir.canonicalize().unwrap();
        let expanded = expand_all(&root);

        group.bench_with_input(
            BenchmarkId::new("build_visible/deep", depth),
            &(&root, &expanded),
            |b, (root, expanded)| {
                let deleted: HashSet<PathBuf> = HashSet::new();
                b.iter(|| black_box(build_visible(root, expanded, false, true, &deleted)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("collect_all_files/deep", depth),
            &root,
            |b, root| b.iter(|| black_box(collect_all_files(root, false, true))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: content search per keystroke (cache-hit path)
// ---------------------------------------------------------------------------

fn bench_content_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("content_search");

    for &(files, lines_per_file) in &[(10usize, 100), (100, 100)] {
        let dir = fixture_dir("search");
        generate_search_files(&dir, files, lines_per_file);

        group.bench_with_input(
            BenchmarkId::new(
                "refresh_content/warm",
                format!("{files}fx{lines_per_file}l"),
            ),
            &dir,
            |b, dir| {
                let mut state = SearchState::new(dir, false, true, 0);
                state.toggle_mode();
                // Warm the cache
                state.push('x');
                state.push('y');
                state.refresh_now();
                state.pop();
                state.pop();

                b.iter(|| {
                    state.push('b');
                    state.push('r'); // 2-char query to trigger content search
                    state.refresh_now();
                    state.pop();
                    state.pop();
                    state.refresh_now();
                })
            },
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: VirtualFile open
// ---------------------------------------------------------------------------

fn bench_file_open(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_open");

    for &lines in &[1_000, 10_000, 100_000] {
        let dir = fixture_dir("open");
        let path = dir.join("large.rs");
        generate_large_rs_file(&path, lines);

        group.bench_with_input(
            BenchmarkId::new("VirtualFile::open", lines),
            &path,
            |b, path| b.iter(|| black_box(VirtualFile::open(path))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: syntax highlight
// ---------------------------------------------------------------------------

fn bench_highlight(c: &mut Criterion) {
    let mut group = c.benchmark_group("highlight");
    let hl = highlighter();

    for &lines in &[100, 1_000, 10_000] {
        let dir = fixture_dir("highlight");
        let path = dir.join("large.rs");
        generate_large_rs_file(&path, lines);

        let vf = VirtualFile::open(&path).unwrap();
        let all_lines: Vec<String> = (0..vf.line_count())
            .filter_map(|i| vf.line_text(i).map(String::from))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("highlight/all", lines),
            &(&path, &all_lines),
            |b, (path, lines)| b.iter(|| black_box(hl.highlight(path, lines))),
        );

        // Simulate a scrolling window of 50 lines
        let window = 50usize;
        if all_lines.len() > window {
            let mid = all_lines.len() / 2;
            let window_slice: Vec<&str> = all_lines[mid..mid + window]
                .iter()
                .map(String::as_str)
                .collect();

            group.bench_with_input(
                BenchmarkId::new("highlight_range/50_window", lines),
                &(&path, &window_slice),
                |b, (path, slice)| b.iter(|| black_box(hl.highlight_range(path, slice))),
            );
        }

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: markdown render
// ---------------------------------------------------------------------------

fn bench_markdown_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("markdown_render");
    let t = theme();

    for &lines in &[100, 1_000, 10_000] {
        let dir = fixture_dir("md");
        let path = dir.join("bench.md");
        generate_large_markdown(&path, lines);
        let content = fs::read_to_string(&path).unwrap();

        group.bench_with_input(
            BenchmarkId::new("render", lines),
            &(&content, &t),
            |b, (src, theme)| b.iter(|| black_box(markdown::render(src, theme))),
        );

        fs::remove_dir_all(&dir).ok();
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark: scroll / redraw — highlight a visible window at different
// positions in a large file.
// ---------------------------------------------------------------------------

fn bench_scroll_redraw(c: &mut Criterion) {
    let mut group = c.benchmark_group("scroll_redraw");
    let hl = highlighter();
    let file_lines = 10_000;

    let dir = fixture_dir("scroll");
    let path = dir.join("large.rs");
    generate_large_rs_file(&path, file_lines);

    let vf = VirtualFile::open(&path).unwrap();
    let all_lines: Vec<String> = (0..vf.line_count())
        .filter_map(|i| vf.line_text(i).map(String::from))
        .collect();
    let refs: Vec<&str> = all_lines.iter().map(String::as_str).collect();

    for &ws in &[25usize, 50] {
        // Top of file
        let top: Vec<&str> = refs[..ws].to_vec();
        group.bench_with_input(
            BenchmarkId::new("highlight_range/top", ws),
            &(&path, &top),
            |b, (path, w)| b.iter(|| black_box(hl.highlight_range(path, w))),
        );

        // Middle
        let mid = refs.len() / 2;
        let middle: Vec<&str> = refs[mid..mid + ws].to_vec();
        group.bench_with_input(
            BenchmarkId::new("highlight_range/mid", ws),
            &(&path, &middle),
            |b, (path, w)| b.iter(|| black_box(hl.highlight_range(path, w))),
        );

        // End
        let end: Vec<&str> = refs[file_lines - ws..].to_vec();
        group.bench_with_input(
            BenchmarkId::new("highlight_range/end", ws),
            &(&path, &end),
            |b, (path, w)| b.iter(|| black_box(hl.highlight_range(path, w))),
        );
    }

    fs::remove_dir_all(&dir).ok();
    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion entry point
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_tree_walk,
    bench_content_search,
    bench_file_open,
    bench_highlight,
    bench_markdown_render,
    bench_scroll_redraw,
);

criterion_main!(benches);
