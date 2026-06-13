# Development

## Prerequisites

- [Rust](https://rustup.rs) (stable toolchain)
- [just](https://github.com/casey/just) (optional, for the justfile recipes)

## Commands

| Command                       | Action                          |
| ----------------------------- | ------------------------------- |
| `cargo build`                 | Debug build                     |
| `cargo build --release`       | Release build                   |
| `cargo run -- [path]`         | Run with optional path          |
| `cargo test`                  | Run all tests                   |
| `cargo check`                 | Type-check only                 |
| `cargo clippy --all-targets -- -D warnings` | Lint                  |
| `cargo fmt --all`             | Format code                     |

Or use the justfile shortcuts:

| Command           | Action                          |
| ----------------- | ------------------------------- |
| `just build`      | Debug build                     |
| `just run .`      | Run against the current directory |
| `just test`       | Run the test suite              |
| `just clippy`     | Lint                            |

## Before committing

1. `cargo fmt --all` — check formatting
2. `cargo clippy --all-targets -- -D warnings` — no warnings
3. `cargo test` — all tests pass
4. `cargo check` — no type errors
5. No debug `println!`, `dbg!`, or commented-out code

## Project structure

```
src/
├── main.rs        # Entry: terminal setup, event loop, dispatch
├── app.rs         # App state, input handling, overlays
├── ui.rs          # ratatui rendering (tree, content, statusbar, popups)
├── tree.rs        # Flat Vec<TreeNode> from ignore::WalkBuilder
├── file.rs        # Binary file detection (null-byte check)
├── config.rs      # tv.toml deserialization, keybinding parsing
├── theme.rs       # Theme struct + 5 presets, color parsing
├── git.rs         # Shells out to `git` for log/diff
├── highlight.rs   # syntect syntax highlighting → ratatui styles
└── markdown.rs    # pulldown-cmark → styled ratatui spans
```
