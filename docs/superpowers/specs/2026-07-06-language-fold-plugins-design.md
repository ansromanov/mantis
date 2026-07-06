# Design: per-language provider plugins (rust/go/python), fold as first capability

Issue: [#483](https://github.com/ansromanov/mantis/issues/483) (first checklist
item of epic #482).

## Context / reversal note

Issue #483 as originally filed proposed a single built-in module
(`src/code_fold.rs`) dispatched by extension in `src/app/loader.rs`, mirroring
`src/yaml_fold.rs`. `docs/src/plugin-capability-matrix.md` (written for the
#296 audit) explicitly recorded that choice: *"Folding for mainstream
languages is going built-in instead (issue #483 ...), so a bundled fold
plugin is deliberately not planned."*

This spec reverses that decision at the requester's direction: language
support is added via plugins, not built into the host. The capability-matrix
doc will be updated as part of this work to remove the "not planned"
language and instead document the 3 new bundled fold plugins as the fold
pipeline's first real bundled consumer.

YAML itself (`src/yaml_fold.rs`) is explicitly **out of scope** here — it
stays built-in as the reference implementation for now. The requester noted
YAML should move to a plugin too, eventually, but as separate follow-up work
(likely its own issue), not bundled into this change.

## Architecture

### Shared detection algorithms (host lib)

New module `src/fold_detectors.rs`, exported from `src/lib.rs`, holds two
pure functions reusing the existing `crate::fold::FoldRegion` type:

- `pub fn brace_fold(text: &str) -> Vec<FoldRegion>` — brace-nesting detector.
  One region per `{…}` block spanning more than one line, tracked with a
  simple nesting stack. Skips braces inside line comments and string
  literals (lexer-lite state machine — same fidelity bar as
  `yaml_fold.rs`, not a real parser). Shared verbatim between Rust and Go
  per the issue's own observation that this detector generalizes across
  brace languages.
- `pub fn indent_fold(text: &str) -> Vec<FoldRegion>` — indentation detector
  for Python. A region runs from each `def`/`class`/compound-statement
  header to the last line more-indented than it; blank lines do not
  terminate a region; `else`/`elif` continuations are treated as belonging
  to the same statement, not a new header.

Both functions are pure (`&str` in, `Vec<FoldRegion>` out) — no IPC, no App
state — so they're unit-testable the same way `yaml_fold.rs` is today, and
callable from any plugin binary that depends on the `mantis` lib crate.

### Three new bundled *language* plugin crates (not fold-specific)

These are **general per-language provider plugins** — `rust`, `go`,
`python` — not single-purpose fold plugins. Fold is the first capability
each implements, because it's the only capability the host currently routes
(`highlight` is formally reserved and flows through syntax plugins instead,
per `plugin-development.md`; `hover`/`diagnostics`/`definition` are reserved
for a future protocol-v3 request/response method). When the host grows a
new capability these plugins can register it too, without restructuring —
they are the Rust/Go/Python language provider, not a "Rust/Go/Python fold
tool" that happens to exist.

Each is a thin process-plugin binary under `plugins/`, following the
`mantis-plugin-markdown` pattern (stdin/stdout JSON loop), added as a new
workspace member:

| Crate | Extensions | Capabilities (today) | Detector called |
|---|---|---|---|
| `plugins/mantis-plugin-rust` | `rs` | `fold` | `mantis::fold_detectors::brace_fold` |
| `plugins/mantis-plugin-go` | `go` | `fold` | `mantis::fold_detectors::brace_fold` |
| `plugins/mantis-plugin-python` | `py`, `pyi` | `fold` | `mantis::fold_detectors::indent_fold` |

Each crate's `Cargo.toml` depends on `mantis = { path = "../.." }` (the
existing `[lib] name = "mantis"` target) plus `serde_json`. Protocol
behavior, all three identical apart from the extension list and which
detector they call:

1. Declare `events = ["on_file_open"]` in `plugin.toml` — no need for
   `on_keypress`/`on_selection_change`/`on_theme_change`, these plugins are
   stateless with respect to theme and don't render content.
2. On `init` → send `register_language_provider` with the extension list and
   `capabilities: ["fold"]`.
3. On `on_file_open` → read the file, run the detector, send
   `set_fold_regions` with the resulting `[start, end]` pairs.
4. On `shutdown`/stdin close → exit.

No `priority` field is needed (default `0`) — nothing else registers `fold`
for `rs`/`go`/`py` since the built-in-dispatch path from the original issue
plan is dropped entirely; there's no collision to break.

### Bundling wiring (host side)

- Add the 3 crates to `[workspace] members` in the root `Cargo.toml`.
- Add 3 entries to `BUNDLED_PLUGINS` in `src/plugin/install.rs`:
  `("rust", "mantis-plugin-rust", ...)`, `("go", "mantis-plugin-go", ...)`,
  `("python", "mantis-plugin-python", ...)`, following the existing
  `(name, binary_name, include_bytes!(...))` shape used for
  `iconize`/`markdown`. Same install path: compiled into the host binary,
  auto-installed to the plugin dir on first run, appear in the plugin
  picker, **default disabled** — user opts in via picker or `mantis.toml`,
  identical to every other bundled plugin today. No special-casing to force
  them on.

## Testing

- **Detector correctness** lives entirely in the host, as
  `src/fold_detectors_test.rs` (colocated `#[cfg(test)]` module, same
  pattern as `fold_test.rs`/`yaml_fold_test.rs`). Covers the issue's full
  acceptance list: nested blocks, braces-in-strings/comments for all three
  languages (Rust raw strings, Go backtick strings, Python triple-quotes),
  Python `else`/`elif` continuation, CRLF line endings.
- **Plugin protocol glue** gets a thin `main_test.rs` per crate (mirroring
  `mantis-plugin-markdown/src/main_test.rs`): feed a small fixture file
  through the `init` → `register_language_provider` → `on_file_open` →
  `set_fold_regions` sequence and assert the emitted JSON shape. These tests
  do not re-verify detector correctness — that's the host's job.
- **Benchmarks**: per the project's hot-path rule (large-input parse code
  gets a `benches/performance.rs` case), add `brace_fold`/`indent_fold`
  cases against large synthetic source files.

## Docs

- `docs/src/plugin-capability-matrix.md`: remove the "bundled fold plugin is
  deliberately not planned" line (§ Gaps and follow-ups item 2); update the
  "Bundled plugins" table to list the 3 new fold plugins; note fold now has
  a real bundled consumer.
- `docs/src/usage.md`: folding section documents that `.rs`/`.go`/`.py`
  folding requires enabling the corresponding bundled plugin (not "works out
  of the box" as originally specified — that acceptance criterion no longer
  applies since folding is opt-in like every other bundled plugin).
- `docs/src/plugin-development.md` / `plugin-registry.md`: add the 3 crates
  to wherever bundled plugins are enumerated.

## Explicitly out of scope

- Migrating `yaml_fold.rs` to a plugin — future follow-up, separate issue.
- Any built-in (non-plugin) fold dispatch in `src/app/loader.rs` — the
  original issue's `code_fold.rs` plan is dropped, not implemented alongside
  the plugins.
