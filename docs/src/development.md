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
├── main.rs             # Entry: terminal setup, event loop, dispatch
├── lib.rs              # Crate root, re-exports the modules below
├── app/                # App state and input handling
│   ├── mod.rs          #   core App state + overlays
│   ├── key_handlers.rs #   keyboard dispatch (incl. open-in-editor)
│   ├── mouse_handlers.rs #  click/scroll handling
│   ├── navigation.rs   #   tree movement & expand/collapse
│   ├── content_pos.rs  #   scroll / wrap position math
│   └── file_ops.rs     #   load files, JSON pretty-print, reloads
├── ui/                 # ratatui rendering
│   ├── mod.rs          #   layout & frame composition
│   ├── tree.rs         #   tree panel
│   ├── content.rs      #   content panel (incl. blame gutter, diffs)
│   ├── popups.rs       #   search / history / palette / help popups
│   └── statusbar.rs    #   status bar
├── config/             # tv.toml deserialization, keybinding parsing
├── command_palette.rs  # Ctrl+P action list + fuzzy matching
├── search.rs           # fuzzy file + full-text content search
├── selection.rs        # text selection model
├── tree.rs             # Flat Vec<TreeNode> from ignore::WalkBuilder
├── file.rs             # Binary file detection (null-byte check)
├── virtual_file.rs     # memory-mapped, lazily indexed large files
├── theme.rs            # Theme struct + presets, color parsing
├── git.rs              # Shells out to `git` for log/diff/blame
├── highlight.rs        # syntect syntax highlighting → ratatui styles
├── markdown.rs         # pulldown-cmark → styled ratatui spans
└── release_info.rs     # bundled "what's new" release metadata
```
