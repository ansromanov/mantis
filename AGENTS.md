# AGENTS.md — tree-viewer (tv)

> Canonical instructions for this repo, read by both Claude Code and opencode.
> Agent assets live in **`.agent/`**, which is symlinked as both `.claude/` and
> `.opencode/` so the two tools share one config + skills directory. `CLAUDE.md`
> defers here and adds only Claude-Code-specific notes.

A fast terminal-based file tree viewer built with ratatui. Navigate filesystems,
preview files with syntax highlighting (`syntect`), render markdown
(`pulldown-cmark`), fuzzy-search files/content (`fuzzy-matcher`), browse git history
(`git` CLI), and switch themes — all with mouse and keyboard.

---

# 1. Project Structure

Single crate, no workspace. Library code in `src/lib.rs`; the `tv` binary in
`src/main.rs`. Tests are co-located in `_test.rs` files (see Rust Guidelines →
Testing).

```
src/
├── main.rs                 # Entry: terminal setup, event loop, dispatch
├── lib.rs                  # Module declarations (crate root)
├── app/
│   ├── mod.rs              # App state, input handling, overlays
│   ├── key_handlers.rs     # Key dispatch to tree/content/search handlers
│   ├── loader.rs           # Background file loader (thread)
│   ├── file_ops.rs         # Open/close/reveal file operations
│   ├── navigation.rs       # Tree navigation helpers
│   └── *_test.rs           # Co-located tests
├── ui/
│   ├── mod.rs              # ratatui rendering orchestration
│   ├── content.rs          # Content panel rendering
│   ├── popups.rs           # Help, search, history, theme picker overlays
│   ├── statusbar.rs        # Status bar rendering
│   ├── tree.rs             # Tree panel rendering
│   └── *_test.rs           # Co-located tests
├── config/
│   ├── mod.rs              # tv.toml deserialization, keybinding parsing
│   └── validate.rs         # Config validation
├── command_palette.rs      # Ctrl-P command palette
├── diff.rs                 # Git diff rendering helpers
├── file.rs                 # Binary file detection (null-byte check)
├── git.rs                  # Shells out to `git` for log/diff
├── highlight.rs            # syntect syntax highlighting → ratatui styles
├── markdown.rs             # pulldown-cmark → styled ratatui spans
├── search.rs               # Fuzzy file/content search (SkimMatcherV2)
├── selection.rs            # Text selection state
├── theme.rs                # Theme struct + presets, color parsing
├── tree.rs                 # Flat Vec<TreeNode> from ignore::WalkBuilder
├── virtual_file.rs         # Virtual file content from highlight/git
└── yaml_fold.rs            # YAML fold-region detection
```

Files grow into the module-directory pattern (`src/app/`, `src/ui/`, `src/config/`)
once they get large: a thin `mod.rs` re-exports focused submodules.

## Key patterns & conventions

1. **Flat tree vector.** The file tree is a `Vec<TreeNode>` with explicit `depth`;
   expansion is tracked in a `HashSet<PathBuf>`. Simpler than nested trees for
   rendering and mouse hit-testing.
2. **Overlay state machine.** Event handlers chain `help` > `theme_picker` >
   `history` > `search` > normal dispatch (tree/content by `Focus`). The same chain
   appears in `handle_mouse()` and `draw()`.
3. **Recorded geometry for mouse.** Each `draw_*` stores its rendered `Rect` and
   scroll offset back on `App`; mouse handlers hit-test with `rect_contains()`.
   **Always account for scroll offsets in click calculations.**
4. **Fuzzy-filterable picker.** `SearchState`, `HistoryState`, `ThemePicker` share a
   shape: query string, full list, filtered+scored list, selected index,
   `push(c)`/`pop()` → `refresh()`. Uses `SkimMatcherV2`, descending score sort.
5. **Semantic theming.** `Theme` is a set of named color roles (not literal colors)
   plus a `syntax` syntect theme name. Presets (default, monokai, solarized,
   catppuccin, synthwave84) live in `PRESETS` plus user overrides; `apply_theme()`
   re-opens the current file after a switch.
6. **Keybinding abstraction.** All actions bind through a `Keymap`; `pressed()`
   checks binding lists. Fully remappable via `tv.toml` `[keys]`.
7. **Git via shell-out.** `git.rs` runs `git log` / `git diff` rather than linking a
   Rust git library, with graceful fallback on failure.
8. **Sync event loop.** `crossterm::event::poll()` with a 16ms timeout — no async
   runtime, just a synchronous tick loop.

---

# 2. Rust Guidelines

## Code style

- **Indent** 4 spaces, no tabs. **Line length** 100 chars max.
- **Naming** snake_case for functions/vars/modules, PascalCase for types/enums.
- **Imports** grouped std → external crates → local modules, separated by blank lines.
  No wildcard imports except `use super::*;` in test files.
- **Doc comments** on all public items. No tautological or self-demonstrating comments.
- **File-level module docs.** Every non-test `.rs` file under `src/` opens with a
  10-15 line `//!` block describing what the module does, the problem it solves, and
  which public items it owns, written for a developer new to the project. Treat keeping
  this block in sync as part of any PR that changes the file's behaviour, the way a
  changelog entry is updated.
- **No emoji/unicode** in source (except test assertions exercising multi-byte handling).
- **Explicit `.clone()`** on non-Copy types — no hidden clones.

## Error handling

`anyhow` only in `main` and `App::new`; use `.context()` for actionable messages.
File and git errors degrade gracefully to UI messages — **no `unwrap`/`expect` in
production paths** (tests may use them freely). Custom errors via `thiserror`.

## Testing

Tests are **co-located** with the module they cover, in a sibling `_test.rs` file —
never inline `#[cfg(test)] mod tests { ... }` blocks.

- `src/foo.rs` → tests in `src/foo_test.rs`
- `src/app/mod.rs` → tests in `src/app/mod_test.rs`

Each `_test.rs` starts with `use super::*;` and contains bare `#[test]` functions
(no `mod tests` wrapper). The source file wires it up with one line:

```rust
#[cfg(test)]
#[path = "foo_test.rs"]
mod tests;
```

When adding tests to an existing module, append to its `_test.rs`. When creating a
new module, create its `_test.rs` companion at the same time. Cross-module /
black-box tests live in the integration `tests/` directory. The `split-tests` skill
(`.agent/skills/split-tests/`) automates extracting any inline block.

## File size limit

Keep every file under **600 lines** (code and tests alike). When a source file
approaches the limit, split it into focused submodules using the module-directory
pattern (`src/app/`, `src/ui/`). When a `_test.rs` approaches it, split by area into
multiple sibling `_test.rs` files.

---

# 3. Dev Flow

## Commands

| Command | Action |
|---|---|
| `cargo build` / `cargo build --release` | Debug / release build |
| `cargo run -- [path]` | Run with optional path |
| `cargo test` | Run all tests |
| `cargo test <name>` | Run specific test |
| `cargo check` | Type-check only |
| `cargo clippy --all-targets -- -D warnings` | Lint (must pass) |
| `cargo fmt --all` / `cargo fmt --check` | Format / check formatting |
| `just` | List all recipes |
| `pre-commit install` | Install git hooks (once after clone) |
| `pre-commit run --all-files` | Run all hooks manually |

## Branching

Always branch from `origin/main`, never from an existing feature branch:

```bash
just new your-branch-name
```

This fetches latest main, creates the branch from `origin/main`, and installs
pre-commit hooks. If you find yourself on a branch not based on main, cherry-pick the
new commits onto a clean branch rather than rebasing through merge noise.

## Opening a PR

```bash
just pr            # fetch origin/main, rebase, push --force-with-lease
gh pr create       # open the PR (rebase fails loudly on conflicts so you can resolve)
```

> **macOS credential helper note:** If `git push` fails with `Device not configured`,
> the macOS keychain isn't available (e.g. in SSH sessions). Workaround:
> ```bash
> GH_TOKEN=$(gh auth token) just pr
> ```

## Before committing

1. `cargo fmt --all` — formatting clean (enforced by pre-commit)
2. `cargo clippy --all-targets -- -D warnings` — no warnings (enforced by pre-commit)
3. `cargo test` — all tests pass
4. `cargo check` — no type errors (enforced by pre-commit)
5. No debug `println!`, `dbg!`, or commented-out code
6. No hardcoded secrets or credentials
