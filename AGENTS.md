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
├── main.rs                         # Entry: terminal setup, sync event loop, dispatch
├── lib.rs                          # Module declarations (crate root)
├── app/
│   ├── mod.rs                      # App struct + shared free functions
│   ├── content_pos.rs              # Content-pane geometry/scroll math (viewport, gutter)
│   ├── content_query.rs            # Read-only line-count/line-text queries (VirtualFile vs raw vs MD vs JSON)
│   ├── diff_nav.rs                 # Jump between @@ hunk headers in diffs
│   ├── file_ops.rs                 # Open/close/reveal file operations
│   ├── key_handlers/
│   │   ├── mod.rs                  # Top-level key dispatch (overlay chain → mode → panel)
│   │   ├── editor.rs               # Key handling when in text-edit/insert mode
│   │   ├── normal.rs               # Key handling in normal (tree/content) mode
│   │   ├── overlay.rs              # Key handling for all overlay popups
│   │   └── visual.rs               # Key handling in visual-selection mode
│   ├── loader.rs                   # Background file-loader thread (LoadRequest/LoadResponse)
│   ├── mouse_handlers.rs           # Mouse event dispatch and click hit-testing
│   ├── navigation.rs               # Tree cursor movement helpers
│   ├── refresh.rs                  # Per-frame tick: drain loads, watcher, debounced search
│   ├── yaml_fold.rs                # YAML fold-region detection (re-export shim)
│   └── *_test.rs                   # Co-located tests
├── ui/
│   ├── mod.rs                      # ratatui rendering orchestration (draw entry point)
│   ├── content/
│   │   ├── mod.rs                  # Content panel draw entry point
│   │   ├── diff.rs                 # Side-by-side and unified diff rendering
│   │   ├── draw.rs                 # Core content rendering (lines, gutter, highlights)
│   │   ├── scrollbar.rs            # Transient fade scrollbar overlay
│   │   ├── search.rs               # In-file search match highlight overlay
│   │   └── selection.rs            # Visual-selection highlight overlay
│   ├── popups/
│   │   ├── mod.rs                  # Re-exports all popup draw functions
│   │   ├── about.rs                # About + release notes overlay
│   │   ├── blame.rs                # Git blame panel for visual-selection range
│   │   ├── command.rs              # Ctrl-P command palette popup
│   │   ├── help.rs                 # ? keybinding help overlay
│   │   ├── history.rs              # Git file-log history overlay
│   │   ├── in_file.rs              # / in-file search bar (bottom of content pane)
│   │   ├── plugin.rs               # Plugin manager overlay
│   │   ├── recent.rs               # Recent-files overlay
│   │   ├── search.rs               # Fuzzy file/content search overlay
│   │   ├── theme.rs                # Theme picker overlay
│   │   └── util.rs                 # Shared popup layout helpers
│   ├── statusbar.rs                # Status bar rendering
│   ├── tree.rs                     # Tree panel rendering
│   └── *_test.rs                   # Co-located tests
├── config/
│   ├── mod.rs                      # tv.toml deserialization, keybinding parsing
│   └── validate.rs                 # Config validation
├── ansi.rs                         # ANSI SGR parser → ratatui Style/Span (for plugin content)
├── command_palette.rs              # CommandPalette + CommandEntry structs and COMMANDS table
├── diff.rs                         # Git diff parse (DiffRow, Cell, parse_side_by_side)
├── file.rs                         # Binary detection, encoding/line-ending probe
├── git.rs                          # Shell-out to git: log, diff, blame, status, repo_info
├── highlight.rs                    # syntect Highlighter → ratatui Style spans
├── markdown.rs                     # pulldown-cmark → styled ratatui spans
├── plugin.rs                       # Plugin, PluginManager, PluginKind, ExtraSyntax; subprocess IPC
├── release_info.rs                 # Compile-time release metadata (ReleaseInfo, RELEASE static)
├── search.rs                       # SearchState, HistoryState, ThemePicker, InFileSearch,
│                                   #   RecentFilesState, PluginPicker (SkimMatcherV2)
├── selection.rs                    # TextSelection + VisualLine for copy/visual mode
├── theme.rs                        # Theme struct, color roles, presets, parse_color
├── tree.rs                         # TreeNode, build_visible (flat Vec from ignore::WalkBuilder)
├── virtual_file.rs                 # Memory-mapped lazily-indexed file (VirtualFile)
└── yaml_fold.rs                    # FoldRegion detection and display-map builder
```

Files grow into the module-directory pattern (`src/app/`, `src/ui/`, `src/config/`)
once they get large: a thin `mod.rs` re-exports focused submodules.

## Key patterns & conventions

1. **Flat tree vector.** The file tree is a `Vec<TreeNode>` with explicit `depth`;
   expansion is tracked in a `HashSet<PathBuf>`. Simpler than nested trees for
   rendering and mouse hit-testing.
2. **Overlay state machine.** Event handlers chain `help` > `about` > `plugin_picker` >
   `recent_files` > `command_palette` > `theme_picker` > `history` > `in_file_search` >
   `search` > normal dispatch (tree/content by `Focus`). The same chain appears in
   `handle_mouse()` and `draw()`.
3. **Recorded geometry for mouse.** Each `draw_*` stores its rendered `Rect` and
   scroll offset back on `App`; mouse handlers hit-test with `rect_contains()`.
   **Always account for scroll offsets in click calculations.**
4. **Fuzzy-filterable picker.** `SearchState`, `HistoryState`, `ThemePicker`,
   `RecentFilesState`, `PluginPicker`, and `CommandPalette` share a shape: query
   string, full list, filtered+scored list, selected index, `push(c)`/`pop()` →
   `refresh()`. Uses `SkimMatcherV2`, descending score sort.
5. **Semantic theming.** `Theme` is a set of named color roles (not literal colors)
   plus a `syntax` syntect theme name. Presets (default, monokai, solarized,
   catppuccin, synthwave84) live in `themes/` plus user overrides; `apply_theme()`
   re-opens the current file after a switch.
6. **Keybinding abstraction.** All actions bind through a `Keymap`; `pressed()`
   checks binding lists. Fully remappable via `tv.toml` `[keys]`.
7. **Git via shell-out.** `git.rs` runs `git log` / `git diff` / `git blame` rather
   than linking a Rust git library, with graceful fallback on failure.
8. **Sync event loop.** `crossterm::event::poll()` with a 16ms timeout — no async
   runtime, just a synchronous tick loop.
9. **Content source abstraction.** `app/content_query.rs` hides whether the active
   content is a `VirtualFile`, `Vec<String>`, JSON pretty-print, or markdown spans;
   everything calls `app.line_count()` / `app.line_text(n)`.
10. **Plugin IPC.** Plugins are external processes communicating over stdin/stdout
    JSON lines (`plugin.rs`). `PluginManager` spawns, kills, and routes actions;
    the content pane can be taken over by plugin-provided ANSI text (`ansi.rs`).

---

## Symbol index

Quick lookup: type/function → file. Use this before grepping.

| Symbol | File | Notes |
|---|---|---|
| `App` | `src/app/mod.rs:74` | Central state struct |
| `Focus` | `src/app/mod.rs:51` | `Tree` / `Content` enum |
| `PluginGitInfo` | `src/app/mod.rs:60` | Plugin-supplied git status |
| `App::tick` | `src/app/refresh.rs` | Per-frame update |
| `App::handle_key` | `src/app/key_handlers/mod.rs` | Top-level key dispatch |
| `App::handle_mouse` | `src/app/mouse_handlers.rs` | Mouse event dispatch |
| `App::open_file` | `src/app/file_ops.rs` | Load a file into the content pane |
| `App::line_count` / `App::line_text` | `src/app/content_query.rs` | Unified content access |
| `App::content_scroll_max` / `App::line_prefix_width` | `src/app/content_pos.rs` | Scroll/gutter math |
| `App::diff_next_hunk` / `App::diff_prev_hunk` | `src/app/diff_nav.rs` | Hunk navigation |
| `Loader` / `LoadRequest` / `LoadResponse` | `src/app/loader.rs` | Background file I/O thread |
| `TreeNode` / `build_visible` | `src/tree.rs` | Flat tree vector |
| `VirtualFile` | `src/virtual_file.rs` | Mmap'd lazily-indexed file |
| `SearchState` | `src/search.rs:36` | Fuzzy file+content search |
| `HistoryState` | `src/search.rs:221` | Git file-log picker |
| `InFileSearch` | `src/search.rs:294` | Within-file incremental search |
| `ThemePicker` | `src/search.rs:357` | Theme selection overlay |
| `RecentFilesState` | `src/search.rs:421` | Recent-files overlay |
| `PluginPicker` | `src/search.rs:489` | Plugin manager overlay |
| `CommandPalette` / `CommandEntry` | `src/command_palette.rs` | Ctrl-P palette |
| `TextSelection` / `VisualLine` | `src/selection.rs` | Visual/copy selection |
| `Theme` / `parse_color` | `src/theme.rs` | Color roles + presets |
| `ThemeConfig` | `src/theme.rs:290` | TOML deserialization target |
| `Highlighter` | `src/highlight.rs:30` | syntect → ratatui styles |
| `DiffRow` / `parse_side_by_side` | `src/diff.rs` | Diff parse/render types |
| `GitRepoInfo` / `GitStatus` / `Commit` / `BlameLine` | `src/git.rs` | Git shell-out types |
| `FoldRegion` / `detect_fold_regions` | `src/yaml_fold.rs` | YAML fold regions |
| `Plugin` / `PluginManager` / `PluginKind` | `src/plugin.rs` | Plugin subprocess IPC |
| `ExtraSyntax` / `PluginEntry` | `src/plugin.rs` | Plugin-registered syntaxes |
| `ReleaseInfo` / `RELEASE` | `src/release_info.rs` | Embedded release metadata |
| `parse_ansi_line` | `src/ansi.rs` | ANSI SGR → ratatui Span |
| `Config` / `Keymap` | `src/config/mod.rs` | tv.toml deserialization |
| `draw` | `src/ui/mod.rs:26` | Main ratatui draw entry point |

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

**Mandatory test rule:** every code change — bug fix, feature, refactor — must
include tests. There are no exceptions. If a change is untestable (e.g. pure UI
paint code), explain why in the PR description. Otherwise add or update tests in
the same commit as the code change, never as a follow-up.

## Documentation

User-facing docs live in `docs/src/`. Any PR that adds, removes, or changes a
user-visible feature (config keys, keybindings, plugin protocol, new UI modes)
**must** update the relevant doc page in the same commit. Treat doc updates the
same way as the `//!` module-doc rule: part of the PR, not a follow-up.

Key pages to consider when changing code:
- Plugin system (`src/plugin.rs`, `src/config/mod.rs`) → `docs/src/plugins.md`,
  `docs/src/plugin-development.md`
- Config options (`src/config/`) → `docs/src/configuration.md`
- Keybindings (`src/config/mod.rs`, `tv.toml`) → `docs/src/configuration.md`
- New UI features → `docs/src/usage.md` or a new page added to `docs/src/SUMMARY.md`

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
| `just test-pr` | **Run only tests related to your changes** (default for PRs) |
| `cargo nextest run` | Run full test suite (use only when `just test-pr` prints "broad change") |
| `cargo nextest run -E 'test(foo)'` | Run tests matching a filter |
| `cargo test` | Run full suite via built-in runner (fallback if nextest unavailable) |
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
3. `just test-pr` — related tests pass (**never run the full suite for a single PR**)
4. `cargo check` — no type errors (enforced by pre-commit)
5. Every changed module has accompanying test changes — no untested diffs
6. No debug `println!`, `dbg!`, or commented-out code
7. No hardcoded secrets or credentials

### How `just test-pr` works

`just test-pr` diffs your branch against `origin/main`, pipes the changed file
list through `scripts/related-tests.sh`, and runs only the matching tests with
`cargo nextest`. If the changes touch broad files (`Cargo.toml`, `src/lib.rs`,
etc.) it automatically falls back to the full suite and prints a notice.

Do **not** run `cargo nextest run` (full suite) or `cargo test` (full suite)
as your default verification step — it wastes minutes and defeats the purpose
of the targeted script. Use `just test-pr` instead.
