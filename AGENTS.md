# AGENTS.md — mantis

> Canonical instructions for this repo, read by Claude Code and opencode
> (`.agent/` is symlinked as `.claude/` and `.opencode/`). Keep this file short:
> it deliberately has **no file tree and no symbol index** — they rot. Use
> `ls`/`grep` and the `//!` module docs at the top of every source file instead.

A fast terminal file-tree viewer built with ratatui: browse a filesystem, preview
files with syntax highlighting (`syntect`), fuzzy-search files and content, browse
git history/diff/blame (via the `git` CLI), switch themes — mouse and keyboard.

Single crate: library in `src/lib.rs`, binary in `src/main.rs`.

## Architecture in one page

- `app/` — the `App` state struct, key/mouse dispatch, file loading, per-frame `tick`.
- `ui/` — ratatui drawing. Each `draw_*` records its `Rect`/scroll offset on `App`
  for mouse hit-testing (always account for scroll offsets in click math).
- `tree.rs` — flat `Vec<TreeNode>` with explicit `depth`; expansion in a `HashSet`.
- `virtual_file.rs` — lazily indexed (mmap above a size threshold) large-file access.
- `app/content_query.rs` — one `line_count()` / `line_text(n)` abstraction over raw
  file, pretty-printed JSON, rendered markdown, etc.
- `git/` — shells out to `git` with graceful fallback; no libgit2.
- `actions.rs` — single registry of actions feeding the command palette, keymap, help.
- `theme.rs` / `themes/` — semantic color roles, not literal colors.

Conventions worth knowing:

1. **Sync event loop.** `crossterm` poll with a 16 ms timeout; no async runtime.
   The loop redraws at full rate only while input is recent and throttles when idle.
2. **Overlay chain.** Key, mouse and draw dispatch all walk the same ordered list of
   overlays before falling through to tree/content. Keep the three in the same order.
3. **Fuzzy-filterable pickers** share a shape (query, items, filtered+scored, selected).
4. **No alt-modified keybindings** — Alt is unreliable across terminals. Use Ctrl,
   Shift (via char case) or unmodified keys only.
5. **Keybindings go through `Keymap`**, remappable in `mantis.toml` `[keys]`.

## Rust guidelines

- 4-space indent, 100-char lines, snake_case / PascalCase, imports grouped
  std → external → local, no wildcard imports (except `use super::*;` in tests).
- Doc comments on public items; every non-test file under `src/` opens with a short
  `//!` block saying what the module does. Update it when behaviour changes.
- **Errors:** `anyhow` only in `main` and `App::new`; file/git errors degrade to a
  status-bar message. No `unwrap`/`expect` in production paths. Don't `let _ =` away
  failures the user expects to see (config save, clipboard, external launch).
- **Per-frame work scales with what is visible**, not total data. Cache by revision.
- **A same-file reload must preserve view state** (scroll, cursor, selection); reset
  only on a genuine content switch.
- **One method per behaviour** — share helpers rather than copy-pasting near-duplicates;
  build the shared primitive when a second consumer is foreseeable.
- Index with `.get(i)` unless the bound is provable. Clamp selection after rebuilds.
- Add a `benches/performance.rs` case for new per-frame or large-input hot paths.
- **Production Rust files stay at or below 800 lines**, enforced by CI
  (`scripts/check-file-size.py`, `just file-size`). Split into a module directory
  (`src/app/`, `src/search/pickers/`) before a file reaches the limit. `*_test.rs`
  companions have no hard limit, but split them alongside the code they test.

## Testing

Tests are co-located in a sibling `_test.rs` (`src/foo.rs` → `src/foo_test.rs`), wired
with `#[cfg(test)] #[path = "foo_test.rs"] mod tests;`; the test file starts with
`use super::*;`. Black-box tests live in `tests/`. Test behaviour through public entry
points (`handle_key`, `handle_mouse`, parsers) rather than poking `App` fields, and use
`tempfile` for temp dirs. Add or update tests with every behaviour change; for pure
paint code say why it is untestable.

## Dev flow

```
just check     # fmt --check, clippy -D warnings, lib tests — same gate as CI
just test      # cargo test
just run PATH  # run the viewer
```

- CI (`.github/workflows/ci.yml`) runs fmt, clippy, the full test suite and an e2e smoke
  test on every PR and push to `main`.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `refactor:`, `ci:`…);
  the release workflow derives the next version from them with `git-cliff`.
- Releases are cut manually from the `Release` workflow; the version lives in `Cargo.toml`
  and is bumped by the release workflow only — never in feature PRs.
- A PR does not need a linked issue. Describe what changed and why.
- User-visible changes update the matching page in `docs/src/`.
