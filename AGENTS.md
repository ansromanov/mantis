# AGENTS.md — tree-viewer (tv)

## Overview

A fast terminal-based file tree viewer with ratatui. Navigate filesystems, preview files with syntax highlighting (`syntect`), render markdown (`pulldown-cmark`), fuzzy-search files/content (`fuzzy-matcher`), browse git history (`git` CLI), and switch themes — all with mouse and keyboard.

## Commands

| Command | Action |
|---|---|
| `cargo build` | Debug build |
| `cargo build --release` | Release build |
| `cargo run -- [path]` | Run with optional path |
| `cargo test` | Run all tests |
| `cargo test -- --nocapture` | Run tests with stdout |
| `cargo test <test_name>` | Run specific test |
| `cargo check` | Type-check only |
| `cargo clippy --all-targets -- -D warnings` | Lint (must pass) |
| `cargo fmt --all` | Format (must pass) |
| `cargo fmt --check` | Check formatting |

## Architecture (single crate, no workspace)

```
src/
├── main.rs        # Entry: terminal setup, event loop, dispatch
├── app.rs         # App state, input handling, overlays (search/history/theme)
├── ui.rs          # ratatui rendering (tree, content, statusbar, popups)
├── tree.rs        # Flat Vec<TreeNode> from ignore::WalkBuilder
├── file.rs        # Binary file detection (null-byte check)
├── config.rs      # tv.toml deserialization, keybinding parsing
├── theme.rs       # Theme struct + 5 presets, color parsing
├── git.rs         # Shells out to `git` for log/diff
├── highlight.rs   # syntect syntax highlighting → ratatui styles
└── markdown.rs    # pulldown-cmark → styled ratatui spans (tables, code blocks, lists)
```

## Key Patterns & Conventions

### 1. Flat tree vector
The file tree is a `Vec<TreeNode>` with explicit `depth`. Expansion tracked in `HashSet<PathBuf>`. Simpler than nested trees for rendering + mouse hit-testing.

### 2. Overlay state machine
Event handler chains: `help` > `theme_picker` > `history` > `search` > normal dispatch (tree/content by `Focus`). Same chain in `handle_mouse()` and `draw()`.

### 3. Recorded geometry for mouse
Each `draw_*` function stores its rendered `Rect` and scroll offset back on `App`. Mouse handlers use `rect_contains()` for hit-testing. **Always account for scroll offsets in click calculations.**

### 4. Fuzzy-filterable picker pattern
`SearchState`, `HistoryState`, `ThemePicker` all share: query string, full list, filtered+scored list, selected index, `push(c)`/`pop()` → `refresh()`. Uses `SkimMatcherV2` with descending score sort.

### 5. Semantic theming
`Theme` has 11 named color roles (not literal colors) plus a `syntax` syntect theme name. Presets (default, monokai, solarized, catppuccin, synthwave84) listed in `PRESETS` + user overrides. `apply_theme()` re-opens current file after theme switch.

### 6. Keybinding abstraction
All actions bound through `Keymap` struct. `pressed()` checks binding lists. Fully remappable via `tv.toml` `[keys]` table.

### 7. Git via shell-out
`git.rs` runs `git log` / `git diff` commands rather than linking a Rust git library. Graceful fallback on failure.

### 8. Error handling
`anyhow` only in `main` and `App::new`. File/git errors degrade gracefully to UI messages. No unwrap/expect in production paths.

### 9. Sync event loop
Uses `crossterm::event::poll()` with 16ms timeout — no async runtime. Simple synchronous tick loop.

## Code Style

- **Indent:** 4 spaces, no tabs
- **Naming:** snake_case for functions/vars/modules, PascalCase for types/enums
- **Imports:** std → external crates → local modules, grouped by blank line
- **No wildcard imports** except in test modules (`use super::*`)
- **Doc comments** on all public items. No tautological or self-demonstrating comments.
- **No emoji/unicode** in source (except in test assertions for multi-byte handling)
- **Line length:** 100 chars max
- **`.clone()` explicitly** on non-Copy types — no hidden clones
- **Custom errors** with `thiserror` / `anyhow` `.context()`

## Before Committing

1. `cargo fmt --all` — formatting clean
2. `cargo clippy --all-targets -- -D warnings` — no warnings
3. `cargo test` — all tests pass
4. `cargo check` — no type errors
5. No debug `println!`, `dbg!`, or commented-out code
6. No hardcoded secrets or credentials
