# Phase 2 — Review Checklist

Use this checklist to perform a detailed review of the PR diff.

## Check Categories

### Correctness & Logic
- Correctness bugs (logic errors, off-by-one, wrong types, unsafe unwraps in production paths)

### AGENTS.md Violations
- `unwrap`/`expect` outside tests
- Alt-modifier keybindings (e.g., `alt+` keys)
- Inline `#[cfg(test)] mod tests { ... }` (must be split → `_test.rs`)
- Missing `//!` module doc blocks on new `.rs` files
- New `set_*` plugin actions without `PluginContributions` entry
- Doc update missing for user-visible feature changes
- **Module split without test split** — if the PR adds new `.rs` submodules extracted from an existing module, the source module's `_test.rs` must be split in the same PR so each new submodule has its own `_test.rs`. Flag as `BUG` if missing.

### Consistency (AGENTS.md → Consistency & performance)
- Duplicated logic — a second near-copy of an existing routine (clipboard copy, editor/browser launch, overlay key handling, scroll clamp) instead of a shared helper
- Ad-hoc scroll/cursor math instead of the canonical helpers (`content_scroll_max`, single clamp path); input and render disagreeing on bounds
- Non-uniform overlay behaviour (missing Esc / empty-Backspace / click-outside close)
- Silently-swallowed user-visible failure (`let _ =` on a config save / clipboard / external launch that should surface a status message)
- Raw `slice[i]` on a derived (non-loop-bounded) index instead of `.get(i)`; selection not clamped after a rebuild

### Performance
- Per-frame `draw_*` doing `O(total)` work/allocation when only the visible window renders (bound it to `view_height`); recompute that should be cached by revision/query
- A reload / watcher tick / plugin re-render that resets scroll/cursor/selection or tears down an open overlay on the *same* content (must guard on a genuine content switch)
- New hot path (per-frame render, large-input parse/search) without a `benches/` case

### Security
- Untrusted input reaching a shell/process: git/plugin args built from file paths or plugin output without escaping; `Command` args from user-controlled data
- Path traversal — reading/writing paths outside the viewed root from plugin or config input
- Terminal/ANSI injection: file content, git output, or plugin output written to the screen without going through the existing sanitiser
- Trusting plugin JSON without validating fields/bounds; unbounded reads from a plugin pipe
- Any hardcoded secret/credential

### Rust Style & Hygiene
- Rust style: line length >100, wildcard imports (except `use super::*;` in tests), missing `.clone()` on non-Copy types
- **Test hygiene:** test function name contradicts what the test actually asserts (e.g. name says "X wins" but assertion message says Y wins or vice versa)
